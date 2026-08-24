use core::{
    cell::{Cell, UnsafeCell},
    fmt,
    marker::PhantomData,
    mem::MaybeUninit,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU16, AtomicU8, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    thread::{self, JoinHandle, Thread},
    time::{Duration, Instant},
};

use spellwire_core::{
    Edge, Injector, InputEvent, Program, SourceFilter, NO_STATE_GATE, TRIGGER_CONSUME,
    TRIGGER_IGNORE_REPEAT,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[allow(dead_code)]
pub enum Capability {
    None = 0,
    HostCallbackInjection = 1 << 0,
    NativeObservation = 1 << 1,
    NativeInjection = 1 << 2,
    NativeOverlay = 1 << 3,
    HostLifecycle = 1 << 4,
    NonBlockingDelay = 1 << 5,
    NativeInputSuppression = 1 << 6,
}

pub const PERMISSION_OBSERVE: u32 = 1 << 0;
pub const PERMISSION_INJECT: u32 = 1 << 1;

#[derive(Debug)]
#[allow(dead_code)]
pub enum PlatformError {
    PermissionDenied(&'static str),
    Initialization(&'static str),
    UnsupportedKey(u16),
    Io(std::io::Error),
    WorkerPanicked,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(permission) => {
                write!(f, "required platform permission is missing: {permission}")
            }
            Self::Initialization(message) => f.write_str(message),
            Self::UnsupportedKey(code) => write!(f, "unsupported USB HID key usage 0x{code:02x}"),
            Self::Io(source) => source.fmt(f),
            Self::WorkerPanicked => f.write_str("platform observer thread panicked"),
        }
    }
}

impl std::error::Error for PlatformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PlatformError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

pub type PlatformInjector = Box<dyn Injector<Error = PlatformError> + Send>;

struct InputSlot(UnsafeCell<MaybeUninit<InputEvent>>);

// SAFETY: The queue has exactly one producer and one consumer. Publication and reuse of each
// slot are ordered by the write/read counters, so the UnsafeCell is never accessed concurrently.
unsafe impl Sync for InputSlot {}

struct InputQueue {
    slots: Box<[InputSlot]>,
    mask: usize,
    write: AtomicUsize,
    read: AtomicUsize,
    producer_closed: AtomicBool,
    consumer_status: AtomicU8,
    consumer_thread: OnceLock<Thread>,
}

const INPUT_CONSUMER_OPEN: u8 = 0;
const INPUT_CONSUMER_OVERFLOW: u8 = 1;
const INPUT_CONSUMER_RESETTING: u8 = 2;
const INPUT_CONSUMER_CLOSED: u8 = 3;

/// Single-producer endpoint used directly by the platform observation callback.
pub(crate) struct InputSender {
    queue: Arc<InputQueue>,
    not_sync: PhantomData<Cell<()>>,
}

/// Single-consumer endpoint owned by the native runtime worker.
pub(crate) struct InputReceiver {
    queue: Arc<InputQueue>,
    not_sync: PhantomData<Cell<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputReceiveError {
    Timeout,
    Disconnected,
    Overflow,
}

/// Creates a fixed-capacity SPSC queue. Capacity must be a power of two.
pub(crate) fn input_channel(capacity: usize) -> (InputSender, InputReceiver) {
    assert!(capacity >= 2 && capacity.is_power_of_two());
    let queue = Arc::new(InputQueue {
        slots: (0..capacity).map(|_| InputSlot(UnsafeCell::new(MaybeUninit::uninit()))).collect(),
        mask: capacity - 1,
        write: AtomicUsize::new(0),
        read: AtomicUsize::new(0),
        producer_closed: AtomicBool::new(false),
        consumer_status: AtomicU8::new(INPUT_CONSUMER_OPEN),
        consumer_thread: OnceLock::new(),
    });
    (
        InputSender { queue: Arc::clone(&queue), not_sync: PhantomData },
        InputReceiver { queue, not_sync: PhantomData },
    )
}

impl InputSender {
    /// Publishes without blocking, allocation, or a mutex. False means full/disconnected.
    pub(crate) fn try_send(&self, event: InputEvent) -> bool {
        if self.queue.consumer_status.load(Ordering::Acquire) != INPUT_CONSUMER_OPEN {
            return false;
        }
        let write = self.queue.write.load(Ordering::Relaxed);
        let read = self.queue.read.load(Ordering::Acquire);
        if write.wrapping_sub(read) >= self.queue.slots.len() {
            let _ = self.queue.consumer_status.compare_exchange(
                INPUT_CONSUMER_OPEN,
                INPUT_CONSUMER_OVERFLOW,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if let Some(worker) = self.queue.consumer_thread.get() {
                worker.unpark();
            }
            return false;
        }
        let slot = &self.queue.slots[write & self.queue.mask];
        // SAFETY: Only this producer writes the slot, and the acquire read above proves the
        // consumer released its previous value before this wraparound reuse.
        unsafe { (*slot.0.get()).write(event) };
        self.queue.write.store(write.wrapping_add(1), Ordering::Release);
        if let Some(worker) = self.queue.consumer_thread.get() {
            worker.unpark();
        }
        true
    }
}

impl Drop for InputSender {
    fn drop(&mut self) {
        self.queue.producer_closed.store(true, Ordering::Release);
        if let Some(worker) = self.queue.consumer_thread.get() {
            worker.unpark();
        }
    }
}

impl InputReceiver {
    fn try_recv(&self) -> Option<InputEvent> {
        let read = self.queue.read.load(Ordering::Relaxed);
        if read == self.queue.write.load(Ordering::Acquire) {
            return None;
        }
        let slot = &self.queue.slots[read & self.queue.mask];
        // SAFETY: Only this consumer reads the slot, and the acquire write counter proves the
        // producer fully initialized this value. InputEvent is Copy and needs no drop handling.
        let event = unsafe { (*slot.0.get()).assume_init_read() };
        self.queue.read.store(read.wrapping_add(1), Ordering::Release);
        Some(event)
    }

    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<InputEvent, InputReceiveError> {
        let current = thread::current();
        let worker = self.queue.consumer_thread.get_or_init(|| current.clone());
        debug_assert_eq!(worker.id(), current.id());
        let deadline = Instant::now() + timeout;
        loop {
            if self.queue.consumer_status.load(Ordering::Acquire) == INPUT_CONSUMER_OVERFLOW
                && self
                    .queue
                    .consumer_status
                    .compare_exchange(
                        INPUT_CONSUMER_OVERFLOW,
                        INPUT_CONSUMER_RESETTING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            {
                let write = self.queue.write.load(Ordering::Acquire);
                self.queue.read.store(write, Ordering::Release);
                self.queue.consumer_status.store(INPUT_CONSUMER_OPEN, Ordering::Release);
                return Err(InputReceiveError::Overflow);
            }
            if let Some(event) = self.try_recv() {
                return Ok(event);
            }
            if self.queue.producer_closed.load(Ordering::Acquire) {
                return Err(InputReceiveError::Disconnected);
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(InputReceiveError::Timeout);
            };
            if remaining.is_zero() {
                return Err(InputReceiveError::Timeout);
            }
            thread::park_timeout(remaining);
        }
    }
}

impl Drop for InputReceiver {
    fn drop(&mut self) {
        self.queue.consumer_status.store(INPUT_CONSUMER_CLOSED, Ordering::Release);
    }
}

const POLICY_SOURCE_COUNT: usize = 2;
const POLICY_DEVICE_COUNT: usize = 2;
const POLICY_EDGE_COUNT: usize = 2;
const POLICY_CODE_COUNT: usize = 256;
const POLICY_REPEAT_COUNT: usize = 2;
const POLICY_LEN: usize = POLICY_SOURCE_COUNT
    * POLICY_DEVICE_COUNT
    * POLICY_EDGE_COUNT
    * POLICY_CODE_COUNT
    * POLICY_REPEAT_COUNT;

/// Lock-free published consume policy read directly by OS hook callbacks.
pub struct InputPolicy {
    entries: Box<[AtomicU16]>,
}

impl InputPolicy {
    #[must_use]
    pub fn new(program: &Program) -> Self {
        let policy = Self { entries: (0..POLICY_LEN).map(|_| AtomicU16::new(0)).collect() };
        policy.update(program, &program.initial_state);
        policy
    }

    pub fn update(&self, program: &Program, state: &[i64]) {
        let mut next = vec![0_u16; POLICY_LEN];
        for handler in &program.handlers {
            let trigger = handler.trigger;
            if trigger.flags & TRIGGER_CONSUME == 0 || !trigger.matches_gate(state) {
                continue;
            }
            let sources: &[usize] = match trigger.source {
                SourceFilter::Physical => &[0],
                SourceFilter::Synthetic => &[1],
                SourceFilter::Any => &[0, 1],
            };
            for &source in sources {
                for repeated in [false, true] {
                    if trigger.edge == Edge::Up && repeated {
                        continue;
                    }
                    if repeated && trigger.flags & TRIGGER_IGNORE_REPEAT != 0 {
                        continue;
                    }
                    for modifiers in 0_u8..16 {
                        if !trigger.matches_context(modifiers, repeated) {
                            continue;
                        }
                        if let Some(index) = policy_index(
                            source,
                            trigger.device as usize,
                            if trigger.edge == Edge::Up {
                                Edge::Down as usize
                            } else {
                                trigger.edge as usize
                            },
                            usize::from(trigger.code),
                            usize::from(repeated),
                        ) {
                            next[index] |= 1_u16 << modifiers;
                        }
                    }
                }
            }
        }
        for (entry, value) in self.entries.iter().zip(next) {
            entry.store(value, Ordering::Release);
        }
    }

    #[must_use]
    pub fn should_consume(&self, event: InputEvent, modifiers: u8, repeated: bool) -> bool {
        let Some(index) = policy_index(
            event.source as usize,
            event.device as usize,
            event.edge as usize,
            usize::from(event.code),
            usize::from(repeated),
        ) else {
            return false;
        };
        self.entries[index].load(Ordering::Acquire) & (1_u16 << (modifiers & 0x0f)) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GateValue {
    slot: u16,
    value: i64,
}

/// Worker-owned cache. Keeps dynamic policy rebuilds off unchanged input events.
pub(crate) struct InputPolicySnapshot {
    gates: Vec<GateValue>,
}

impl InputPolicySnapshot {
    pub(crate) fn new(program: &Program, state: &[i64]) -> Self {
        let mut slots: Vec<u16> = program
            .handlers
            .iter()
            .map(|handler| handler.trigger.gate)
            .filter(|slot| *slot != NO_STATE_GATE)
            .collect();
        slots.sort_unstable();
        slots.dedup();
        let gates = slots
            .into_iter()
            .map(|slot| GateValue {
                slot,
                value: state.get(usize::from(slot)).copied().unwrap_or_default(),
            })
            .collect();
        Self { gates }
    }

    pub(crate) fn synchronize(&mut self, policy: &InputPolicy, program: &Program, state: &[i64]) {
        let changed = self.gates.iter().any(|gate| {
            state.get(usize::from(gate.slot)).copied().unwrap_or_default() != gate.value
        });
        if !changed {
            return;
        }
        policy.update(program, state);
        for gate in &mut self.gates {
            gate.value = state.get(usize::from(gate.slot)).copied().unwrap_or_default();
        }
    }

    pub(crate) fn replace(&mut self, policy: &InputPolicy, program: &Program, state: &[i64]) {
        policy.update(program, state);
        *self = Self::new(program, state);
    }
}

fn policy_index(
    source: usize,
    device: usize,
    edge: usize,
    code: usize,
    repeated: usize,
) -> Option<usize> {
    if source >= POLICY_SOURCE_COUNT
        || device >= POLICY_DEVICE_COUNT
        || edge >= POLICY_EDGE_COUNT
        || code >= POLICY_CODE_COUNT
        || repeated >= POLICY_REPEAT_COUNT
    {
        return None;
    }
    Some(
        ((((source * POLICY_DEVICE_COUNT + device) * POLICY_EDGE_COUNT + edge)
            * POLICY_CODE_COUNT
            + code)
            * POLICY_REPEAT_COUNT)
            + repeated,
    )
}

pub struct Observer {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), PlatformError>>>,
    wake: Option<Box<dyn FnOnce() + Send>>,
}

impl Observer {
    #[cfg(not(target_os = "windows"))]
    pub(crate) fn new(stop: Arc<AtomicBool>, join: JoinHandle<Result<(), PlatformError>>) -> Self {
        Self { stop, join: Some(join), wake: None }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn new_with_wake(
        stop: Arc<AtomicBool>,
        join: JoinHandle<Result<(), PlatformError>>,
        wake: Box<dyn FnOnce() + Send>,
    ) -> Self {
        Self { stop, join: Some(join), wake: Some(wake) }
    }

    /// Stops the platform observer and joins its thread.
    ///
    /// # Errors
    ///
    /// Returns a platform error when shutdown fails or the observer thread panics.
    pub fn stop(mut self) -> Result<(), PlatformError> {
        self.stop.store(true, Ordering::Release);
        if let Some(wake) = self.wake.take() {
            wake();
        }
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        join.join().map_err(|_| PlatformError::WorkerPanicked)?
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(wake) = self.wake.take() {
            wake();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[must_use]
pub const fn current_capabilities() -> u32 {
    let capabilities = Capability::HostCallbackInjection as u32
        | Capability::NativeObservation as u32
        | Capability::NativeInjection as u32
        | Capability::HostLifecycle as u32
        | Capability::NonBlockingDelay as u32;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        capabilities | Capability::NativeInputSuppression as u32
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        capabilities
    }
}

/// Creates the current operating system's native input injector.
///
/// # Errors
///
/// Returns a platform error when permissions, devices, or native APIs are unavailable.
pub fn create_injector() -> Result<PlatformInjector, PlatformError> {
    backend::create_injector()
}

/// Starts global native input observation and publishes events into `sender`.
///
/// # Errors
///
/// Returns a platform error when permissions, devices, hooks, or event taps are unavailable.
pub(crate) fn start_observer(
    sender: InputSender,
    policy: Arc<InputPolicy>,
) -> Result<Observer, PlatformError> {
    backend::start_observer(sender, policy)
}

#[must_use]
pub fn permission_status() -> u32 {
    backend::permission_status()
}

#[must_use]
pub fn request_permissions() -> u32 {
    backend::request_permissions()
}

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "windows")]
use windows as backend;

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use spellwire_core::{
        key, Edge, Handler, InputDevice, InputEvent, InputSource, Instruction, Opcode, Program,
        SourceFilter, Trigger, MODIFIER_CONTROL, MODIFIER_SHIFT, TRIGGER_CONSUME,
        TRIGGER_EXACT_MODIFIERS, TRIGGER_IGNORE_REPEAT,
    };

    use super::{input_channel, InputPolicy, InputPolicySnapshot, InputReceiveError};

    #[test]
    fn spsc_input_queue_is_bounded_ordered_and_disconnect_aware() {
        let (sender, receiver) = input_channel(2);
        let first = InputEvent {
            device: InputDevice::Keyboard,
            code: key::A,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        let second = InputEvent { code: key::B, ..first };
        let overflow = InputEvent { code: key::C, ..first };

        assert!(sender.try_send(first));
        assert!(sender.try_send(second));
        assert!(!sender.try_send(overflow));
        assert!(!sender.try_send(first));
        assert_eq!(receiver.recv_timeout(Duration::ZERO), Err(InputReceiveError::Overflow));
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(1)),
            Err(InputReceiveError::Timeout)
        );
        assert!(sender.try_send(first));
        assert_eq!(receiver.recv_timeout(Duration::ZERO), Ok(first));
        drop(sender);
        assert_eq!(
            receiver.recv_timeout(Duration::from_millis(1)),
            Err(InputReceiveError::Disconnected)
        );
    }

    #[test]
    fn spsc_input_queue_wakes_the_waiting_consumer() {
        let (sender, receiver) = input_channel(2);
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::K,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        let worker = thread::spawn(move || receiver.recv_timeout(Duration::from_secs(1)));
        thread::yield_now();
        assert!(sender.try_send(event));
        assert_eq!(worker.join().unwrap(), Ok(event));
    }

    #[test]
    fn spsc_input_queue_preserves_order_across_wraparound() {
        const EVENTS: usize = 100_000;
        let (sender, receiver) = input_channel(8);
        for batch in 0..(EVENTS / 8) {
            for offset in 0..8 {
                let index = batch * 8 + offset;
                let event = InputEvent {
                    device: InputDevice::Keyboard,
                    code: u16::try_from(index % 256).unwrap(),
                    edge: Edge::Down,
                    source: InputSource::Physical,
                };
                assert!(sender.try_send(event));
            }
            for offset in 0..8 {
                let index = batch * 8 + offset;
                let event = receiver.recv_timeout(Duration::ZERO).unwrap();
                assert_eq!(event.code, u16::try_from(index % 256).unwrap());
            }
        }
    }

    #[test]
    fn consume_policy_matches_source_modifiers_and_repeat_without_locks() {
        let program = Program {
            initial_state: Box::new([]),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: TRIGGER_CONSUME | TRIGGER_EXACT_MODIFIERS | TRIGGER_IGNORE_REPEAT,
                    modifiers: MODIFIER_CONTROL,
                    gate: spellwire_core::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![Instruction::new(Opcode::Halt)].into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let policy = InputPolicy::new(&program);
        let physical = InputEvent {
            device: InputDevice::Keyboard,
            code: key::K,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        let synthetic = InputEvent { source: InputSource::Synthetic, ..physical };

        assert!(policy.should_consume(physical, MODIFIER_CONTROL, false));
        assert!(!policy.should_consume(physical, MODIFIER_CONTROL | MODIFIER_SHIFT, false));
        assert!(!policy.should_consume(physical, MODIFIER_CONTROL, true));
        assert!(!policy.should_consume(synthetic, MODIFIER_CONTROL, false));
    }

    #[test]
    fn state_gate_rebuilds_consume_table_only_after_value_changes() {
        let program = Program {
            initial_state: vec![0].into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: TRIGGER_CONSUME,
                    modifiers: 0,
                    gate: 0,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![Instruction::new(Opcode::Halt)].into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let policy = InputPolicy::new(&program);
        let mut snapshot = InputPolicySnapshot::new(&program, &[0]);
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: key::K,
            edge: Edge::Down,
            source: InputSource::Physical,
        };

        assert!(!policy.should_consume(event, 0, false));
        snapshot.synchronize(&policy, &program, &[1]);
        assert!(policy.should_consume(event, 0, false));
        snapshot.synchronize(&policy, &program, &[0]);
        assert!(!policy.should_consume(event, 0, false));
    }

    #[test]
    fn consuming_release_arms_on_down_and_relies_on_paired_release() {
        let program = Program {
            initial_state: Box::new([]),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::K,
                    edge: Edge::Up,
                    source: SourceFilter::Physical,
                    flags: TRIGGER_CONSUME,
                    modifiers: 0,
                    gate: spellwire_core::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![Instruction::new(Opcode::Halt)].into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let policy = InputPolicy::new(&program);
        let down = InputEvent {
            device: InputDevice::Keyboard,
            code: key::K,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        let up = InputEvent { edge: Edge::Up, ..down };

        assert!(policy.should_consume(down, 0, false));
        assert!(!policy.should_consume(up, 0, false));
    }
}
