use core::{ffi::c_char, mem, ptr, ptr::NonNull, slice};
use std::{
    sync::{
        atomic::{AtomicI32, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use spellwire_core::{
    ContinuationScheduler, Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton,
    OutputEvent, Program, Runtime, RuntimeConfig,
};

use crate::platform::{self, Observer, PlatformError, PlatformInjector};

const INPUT_CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CHANNEL_CAPACITY: usize = 64;
const MAX_COMMAND_LATENCY: Duration = Duration::from_millis(5);

const STATUS_OK: i32 = 0;
const STATUS_NULL: i32 = -1;
const STATUS_INVALID_ARGUMENT: i32 = -2;
const STATUS_RUNTIME: i32 = -5;
const STATUS_NOT_RUNNING: i32 = -7;
const STATUS_ALREADY_RUNNING: i32 = -8;
const STATUS_PLATFORM: i32 = -9;
const STATUS_CHANNEL: i32 = -10;
const STATUS_SCHEDULER_FULL: i32 = -11;
const DYNAMIC_RING_HEADER_WORDS: usize = 4;
const DYNAMIC_RING_RECORD_WORDS: usize = 6;
const DYNAMIC_RING_WRITE: usize = 0;
const DYNAMIC_RING_READ: usize = 1;
const DYNAMIC_RING_DROPPED: usize = 2;
const DYNAMIC_RING_CLOSED: usize = 3;

pub struct SpellwireHost {
    state: Mutex<HostState>,
    last_error: Arc<Mutex<String>>,
}

struct HostState {
    program: Program,
    running: Option<RunningHost>,
}

struct RunningHost {
    commands: SyncSender<HostCommand>,
    observer: Option<Observer>,
    worker: Option<JoinHandle<()>>,
}

enum HostCommand {
    Stop(SyncSender<i32>),
    GetState { slot: usize, reply: SyncSender<Option<i64>> },
    SetState { slot: usize, value: i64, reply: SyncSender<bool> },
    SnapshotState { output: StateSnapshot, reply: SyncSender<i32> },
    Reload { program: Program, preserve_state: bool, reply: SyncSender<i32> },
    Dispatch { event: InputEvent, reply: SyncSender<i32> },
    SetInputRing { ring: Option<DynamicRing>, reply: SyncSender<()> },
}

struct StateSnapshot {
    output: NonNull<i64>,
    capacity: usize,
}

// SAFETY: The FFI call is synchronous and retains the host-provided output buffer until the
// worker replies. Only the worker writes it during that interval.
unsafe impl Send for StateSnapshot {}

impl StateSnapshot {
    fn write(self, state: &[i64]) -> i32 {
        if self.capacity < state.len() {
            return STATUS_INVALID_ARGUMENT;
        }
        // SAFETY: Construction validates a non-null output pointer. The synchronous FFI contract
        // keeps capacity writable until the worker reply is received.
        unsafe { ptr::copy_nonoverlapping(state.as_ptr(), self.output.as_ptr(), state.len()) };
        STATUS_OK
    }
}

struct DynamicRing {
    words: NonNull<AtomicI32>,
    capacity: u32,
    mask: u32,
}

// SAFETY: The pointer targets SharedArrayBuffer storage retained by the Bun host. Every native
// access is atomic, and the synchronous detach/stop APIs finish before Bun releases that storage.
unsafe impl Send for DynamicRing {}

impl DynamicRing {
    unsafe fn new(words: *mut i32, word_len: usize, capacity: usize) -> Option<Self> {
        if words.is_null()
            || words.align_offset(mem::align_of::<AtomicI32>()) != 0
            || capacity < 2
            || capacity > i32::MAX as usize
            || !capacity.is_power_of_two()
            || word_len
                != DYNAMIC_RING_HEADER_WORDS
                    .checked_add(capacity.checked_mul(DYNAMIC_RING_RECORD_WORDS)?)?
        {
            return None;
        }
        Some(Self {
            words: NonNull::new(words.cast())?,
            capacity: u32::try_from(capacity).ok()?,
            mask: u32::try_from(capacity - 1).ok()?,
        })
    }

    fn push(&self, event: InputEvent) {
        let words = self.words.as_ptr();
        // SAFETY: Construction validates the complete ring buffer and the FFI contract keeps it
        // live until a synchronous detach or host stop. All indexes below are in that buffer.
        unsafe {
            if (*words.add(DYNAMIC_RING_CLOSED)).load(Ordering::Acquire) != 0 {
                return;
            }
            let write = u32::from_ne_bytes(
                (*words.add(DYNAMIC_RING_WRITE)).load(Ordering::Relaxed).to_ne_bytes(),
            );
            let read = u32::from_ne_bytes(
                (*words.add(DYNAMIC_RING_READ)).load(Ordering::Acquire).to_ne_bytes(),
            );
            if write.wrapping_sub(read) >= self.capacity {
                (*words.add(DYNAMIC_RING_DROPPED)).fetch_add(1, Ordering::Relaxed);
                return;
            }
            let base = DYNAMIC_RING_HEADER_WORDS
                + usize::try_from(write & self.mask).unwrap_or_default()
                    * DYNAMIC_RING_RECORD_WORDS;
            let timestamp = monotonic_timestamp_ns();
            let timestamp_bytes = timestamp.to_le_bytes();
            let record = [
                event.device as i32,
                i32::from(event.code),
                event.edge as i32,
                event.source as i32,
                i32::from_le_bytes(timestamp_bytes[0..4].try_into().unwrap_or_default()),
                i32::from_le_bytes(timestamp_bytes[4..8].try_into().unwrap_or_default()),
            ];
            for (index, value) in record.into_iter().enumerate() {
                (*words.add(base + index)).store(value, Ordering::Relaxed);
            }
            (*words.add(DYNAMIC_RING_WRITE))
                .store(i32::from_ne_bytes(write.wrapping_add(1).to_ne_bytes()), Ordering::Release);
        }
    }
}

fn monotonic_timestamp_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    u64::try_from(START.get_or_init(Instant::now).elapsed().as_nanos()).unwrap_or(u64::MAX)
}

struct TrackingInjector {
    inner: PlatformInjector,
    keys: [bool; 256],
    mouse: [bool; 5],
}

impl TrackingInjector {
    fn new(inner: PlatformInjector) -> Self {
        Self { inner, keys: [false; 256], mouse: [false; 5] }
    }

    fn release_all(&mut self) -> Result<(), PlatformError> {
        let mut releases = Vec::new();
        for (code, down) in self.keys.iter().copied().enumerate() {
            if down {
                releases.push(OutputEvent::Key {
                    code: u16::try_from(code).unwrap_or_default(),
                    down: false,
                });
            }
        }
        for (button, down) in self.mouse.iter().copied().enumerate() {
            if down {
                let Ok(button) = u8::try_from(button) else { continue };
                let Ok(button) = MouseButton::try_from(button) else { continue };
                releases.push(OutputEvent::MouseButton { button, down: false });
            }
        }
        if !releases.is_empty() {
            self.inner.send(&releases)?;
        }
        self.keys.fill(false);
        self.mouse.fill(false);
        Ok(())
    }
}

impl Injector for TrackingInjector {
    type Error = PlatformError;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        // Mark intended downs before submission. If a backend partially submits then fails,
        // `release_all` will send harmless ups for every key/button that might have gone down.
        for event in events {
            match *event {
                OutputEvent::Key { code, down: true } if code < 256 => {
                    self.keys[usize::from(code)] = true;
                }
                OutputEvent::MouseButton { button, down: true } => {
                    self.mouse[button as usize] = true;
                }
                _ => {}
            }
        }
        self.inner.send(events)?;
        for event in events {
            match *event {
                OutputEvent::Key { code, down: false } if code < 256 => {
                    self.keys[usize::from(code)] = false;
                }
                OutputEvent::MouseButton { button, down: false } => {
                    self.mouse[button as usize] = false;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl RunningHost {
    fn stop(mut self, last_error: &Mutex<String>) -> i32 {
        let mut status = STATUS_OK;
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        let _ = self.commands.send(HostCommand::Stop(reply_sender));
        match reply_receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(worker_status) if worker_status != STATUS_OK => status = worker_status,
            Ok(_) => {}
            Err(error) => {
                set_worker_error(last_error, &error.to_string());
                status = STATUS_CHANNEL;
            }
        }
        if let Some(observer) = self.observer.take() {
            if let Err(error) = observer.stop() {
                set_worker_error(last_error, &error.to_string());
                status = STATUS_PLATFORM;
            }
        }
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                set_worker_error(last_error, "native runtime worker panicked during shutdown");
                status = STATUS_PLATFORM;
            }
        }
        status
    }
}

impl SpellwireHost {
    fn new(program: Program) -> Result<Self, ()> {
        Runtime::new(program.clone(), RuntimeConfig::default()).map_err(|_| ())?;
        Ok(Self {
            state: Mutex::new(HostState { program, running: None }),
            last_error: Arc::new(Mutex::new(String::new())),
        })
    }

    fn set_error(&self, message: impl Into<String>) {
        if let Ok(mut error) = self.last_error.lock() {
            *error = message.into();
        }
    }

    fn command_sender(&self) -> Result<SyncSender<HostCommand>, i32> {
        let state = self.state.lock().map_err(|_| STATUS_RUNTIME)?;
        state.running.as_ref().map(|running| running.commands.clone()).ok_or(STATUS_NOT_RUNNING)
    }

    fn start(&self) -> i32 {
        let Ok(mut state) = self.state.lock() else { return STATUS_RUNTIME };
        if state.running.is_some() {
            return STATUS_ALREADY_RUNNING;
        }

        let injector = match platform::create_injector() {
            Ok(injector) => injector,
            Err(error) => {
                self.set_error(error.to_string());
                return STATUS_PLATFORM;
            }
        };
        let (input_sender, input_receiver) = mpsc::sync_channel(INPUT_CHANNEL_CAPACITY);
        let observer = match platform::start_observer(input_sender) {
            Ok(observer) => observer,
            Err(error) => {
                self.set_error(error.to_string());
                return STATUS_PLATFORM;
            }
        };
        let (command_sender, command_receiver) = mpsc::sync_channel(COMMAND_CHANNEL_CAPACITY);
        let program = state.program.clone();
        let worker_error = Arc::clone(&self.last_error);
        let worker =
            match thread::Builder::new().name("spellwire-runtime".into()).spawn(move || {
                run_worker(program, injector, input_receiver, command_receiver, &worker_error);
            }) {
                Ok(worker) => worker,
                Err(error) => {
                    drop(observer);
                    self.set_error(error.to_string());
                    return STATUS_PLATFORM;
                }
            };
        state.running = Some(RunningHost {
            commands: command_sender,
            observer: Some(observer),
            worker: Some(worker),
        });
        self.set_error(String::new());
        STATUS_OK
    }

    fn stop(&self) -> i32 {
        let running = {
            let Ok(mut state) = self.state.lock() else { return STATUS_RUNTIME };
            state.running.take()
        };
        let Some(running) = running else {
            return STATUS_NOT_RUNNING;
        };
        running.stop(&self.last_error)
    }
}

impl Drop for SpellwireHost {
    fn drop(&mut self) {
        if let Ok(state) = self.state.get_mut() {
            if let Some(running) = state.running.take() {
                let _ = running.stop(&self.last_error);
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_worker(
    program: Program,
    injector: PlatformInjector,
    inputs: Receiver<InputEvent>,
    commands: Receiver<HostCommand>,
    last_error: &Mutex<String>,
) {
    let Ok(mut runtime) = Runtime::new(program, RuntimeConfig::default()) else {
        set_worker_error(last_error, "validated program failed runtime initialization");
        return;
    };
    let mut injector = TrackingInjector::new(injector);
    let mut scheduler = ContinuationScheduler::default();
    let mut input_ring: Option<DynamicRing> = None;

    'worker: loop {
        loop {
            match commands.try_recv() {
                Ok(HostCommand::Stop(reply)) => {
                    scheduler.clear();
                    let status = match injector.release_all() {
                        Ok(()) => STATUS_OK,
                        Err(error) => {
                            set_worker_error(last_error, &error.to_string());
                            STATUS_PLATFORM
                        }
                    };
                    let _ = reply.send(status);
                    break 'worker;
                }
                Ok(HostCommand::GetState { slot, reply }) => {
                    let _ = reply.send(runtime.get_state(slot));
                }
                Ok(HostCommand::SetState { slot, value, reply }) => {
                    let _ = reply.send(runtime.set_state(slot, value));
                }
                Ok(HostCommand::SnapshotState { output, reply }) => {
                    let _ = reply.send(output.write(runtime.state()));
                }
                Ok(HostCommand::Reload { program, preserve_state, reply }) => {
                    let status = reload_runtime(
                        &mut runtime,
                        &mut scheduler,
                        &mut injector,
                        program,
                        preserve_state,
                        last_error,
                    );
                    let _ = reply.send(status);
                }
                Ok(HostCommand::Dispatch { event, reply }) => {
                    if let Some(ring) = &input_ring {
                        ring.push(event);
                    }
                    let status = enqueue_and_poll(
                        &mut runtime,
                        &mut scheduler,
                        event,
                        &mut injector,
                        last_error,
                    );
                    let _ = reply.send(status);
                }
                Ok(HostCommand::SetInputRing { ring, reply }) => {
                    input_ring = ring;
                    let _ = reply.send(());
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'worker,
            }
        }

        let now = Instant::now();
        if let Err(error) = runtime.poll_ready(&mut scheduler, now, &mut injector) {
            set_worker_error(last_error, &error.to_string());
            if let Err(release_error) = injector.release_all() {
                set_worker_error(last_error, &release_error.to_string());
            }
        }
        let timeout = scheduler.next_deadline().map_or(MAX_COMMAND_LATENCY, |deadline| {
            deadline
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::ZERO)
                .min(MAX_COMMAND_LATENCY)
        });
        match inputs.recv_timeout(timeout) {
            Ok(event) => {
                if let Some(ring) = &input_ring {
                    ring.push(event);
                }
                let _ = enqueue_and_poll(
                    &mut runtime,
                    &mut scheduler,
                    event,
                    &mut injector,
                    last_error,
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                set_worker_error(last_error, "platform observer input channel disconnected");
                break;
            }
        }
    }
    if let Err(error) = injector.release_all() {
        set_worker_error(last_error, &error.to_string());
    }
}

fn enqueue_and_poll(
    runtime: &mut Runtime,
    scheduler: &mut ContinuationScheduler,
    event: InputEvent,
    injector: &mut TrackingInjector,
    last_error: &Mutex<String>,
) -> i32 {
    let now = Instant::now();
    if let Err(error) = runtime.enqueue(event, scheduler, now) {
        set_worker_error(last_error, &error.to_string());
        return STATUS_SCHEDULER_FULL;
    }
    match runtime.poll_ready(scheduler, now, injector) {
        Ok(_) => STATUS_OK,
        Err(error) => {
            set_worker_error(last_error, &error.to_string());
            if let Err(release_error) = injector.release_all() {
                set_worker_error(last_error, &release_error.to_string());
            }
            STATUS_RUNTIME
        }
    }
}

fn reload_runtime(
    runtime: &mut Runtime,
    scheduler: &mut ContinuationScheduler,
    injector: &mut TrackingInjector,
    program: Program,
    preserve_state: bool,
    last_error: &Mutex<String>,
) -> i32 {
    let Ok(mut replacement) = Runtime::new(program, RuntimeConfig::default()) else {
        return STATUS_RUNTIME;
    };
    if preserve_state {
        for (slot, value) in runtime.state().iter().copied().enumerate() {
            if !replacement.set_state(slot, value) {
                break;
            }
        }
    }
    if let Err(error) = injector.release_all() {
        set_worker_error(last_error, &error.to_string());
        return STATUS_PLATFORM;
    }
    scheduler.clear();
    *runtime = replacement;
    STATUS_OK
}

fn set_worker_error(last_error: &Mutex<String>, message: &str) {
    if let Ok(mut error) = last_error.lock() {
        message.clone_into(&mut error);
    }
}

fn decode_program(bytes: *const u8, len: usize) -> Result<Program, i32> {
    if bytes.is_null() || len == 0 {
        return Err(STATUS_INVALID_ARGUMENT);
    }
    // SAFETY: Callers of the C entry points promise a readable byte buffer for the call.
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    Program::decode(bytes).map_err(|_| STATUS_INVALID_ARGUMENT)
}

fn send_command<T>(
    sender: &SyncSender<HostCommand>,
    command: HostCommand,
    reply: &Receiver<T>,
) -> Result<T, i32> {
    sender.send(command).map_err(|_| STATUS_CHANNEL)?;
    reply.recv().map_err(|_| STATUS_CHANNEL)
}

/// Creates a stopped native platform host from a complete bytecode buffer.
///
/// # Safety
///
/// `bytes` must point to `len` readable bytes for this call. The returned pointer must be freed
/// exactly once with [`spellwire_host_free`].
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_new(bytes: *const u8, len: usize) -> *mut SpellwireHost {
    let Ok(program) = decode_program(bytes, len) else {
        return ptr::null_mut();
    };
    match SpellwireHost::new(program) {
        Ok(host) => Box::into_raw(Box::new(host)),
        Err(()) => ptr::null_mut(),
    }
}

/// Stops and releases a native platform host.
///
/// # Safety
///
/// `host` must be null or a live pointer returned by [`spellwire_host_new`], and freeing must not
/// race another host API call.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_free(host: *mut SpellwireHost) {
    if host.is_null() {
        return;
    }
    // SAFETY: The caller returns unique ownership of the live host allocation.
    unsafe { drop(Box::from_raw(host)) };
}

/// Starts global observation and native injection.
///
/// # Safety
///
/// `host` must point to a live host and must not be freed concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_start(host: *mut SpellwireHost) -> i32 {
    // SAFETY: The caller promises a live pointer; null is checked before dereference.
    unsafe { host.as_ref() }.map_or(STATUS_NULL, SpellwireHost::start)
}

/// Stops the observer and runtime worker, releasing any synthetic held inputs.
///
/// # Safety
///
/// `host` must point to a live host and must not be freed concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_stop(host: *mut SpellwireHost) -> i32 {
    // SAFETY: The caller promises a live pointer; null is checked before dereference.
    unsafe { host.as_ref() }.map_or(STATUS_NULL, SpellwireHost::stop)
}

/// Replaces the active program and optionally copies common numeric state slots.
///
/// # Safety
///
/// `host` and `bytes` must remain readable for this call and the host must not be freed
/// concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_reload(
    host: *mut SpellwireHost,
    bytes: *const u8,
    len: usize,
    preserve_state: bool,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else {
        return STATUS_NULL;
    };
    let program = match decode_program(bytes, len) {
        Ok(program) => program,
        Err(status) => return status,
    };
    if Runtime::new(program.clone(), RuntimeConfig::default()).is_err() {
        return STATUS_INVALID_ARGUMENT;
    }
    let sender = {
        let Ok(mut state) = host.state.lock() else {
            return STATUS_RUNTIME;
        };
        let Some(running) = state.running.as_ref() else {
            state.program = program;
            return STATUS_OK;
        };
        running.commands.clone()
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command =
        HostCommand::Reload { program: program.clone(), preserve_state, reply: reply_sender };
    let status = match send_command(&sender, command, &reply_receiver) {
        Ok(status) | Err(status) => status,
    };
    if status == STATUS_OK {
        if let Ok(mut state) = host.state.lock() {
            state.program = program;
        }
    }
    status
}

/// Dispatches one event through the live host worker. Primarily useful for deterministic tests.
///
/// # Safety
///
/// `host` must point to a live host and must not be freed concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_dispatch(
    host: *mut SpellwireHost,
    device: u8,
    code: u16,
    edge: u8,
    source: u8,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else {
        return STATUS_NULL;
    };
    let Ok(device) = InputDevice::try_from(device) else { return STATUS_INVALID_ARGUMENT };
    let Ok(edge) = Edge::try_from(edge) else { return STATUS_INVALID_ARGUMENT };
    let Ok(source) = InputSource::try_from(source) else { return STATUS_INVALID_ARGUMENT };
    let sender = match host.command_sender() {
        Ok(sender) => sender,
        Err(status) => return status,
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command = HostCommand::Dispatch {
        event: InputEvent { device, code, edge, source },
        reply: reply_sender,
    };
    match send_command(&sender, command, &reply_receiver) {
        Ok(status) | Err(status) => status,
    }
}

/// Reads one state slot from the live worker.
///
/// # Safety
///
/// `host` and `output` must be live, non-overlapping pointers for this call.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_state_get(
    host: *const SpellwireHost,
    slot: usize,
    output: *mut i64,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else { return STATUS_NULL };
    if output.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    let sender = match host.command_sender() {
        Ok(sender) => sender,
        Err(status) => return status,
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command = HostCommand::GetState { slot, reply: reply_sender };
    match send_command(&sender, command, &reply_receiver) {
        Ok(Some(value)) => {
            // SAFETY: The caller promises a writable output slot.
            unsafe { output.write(value) };
            STATUS_OK
        }
        Ok(None) => STATUS_INVALID_ARGUMENT,
        Err(status) => status,
    }
}

/// Writes one state slot on the live worker.
///
/// # Safety
///
/// `host` must point to a live host and must not be freed concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_state_set(
    host: *mut SpellwireHost,
    slot: usize,
    value: i64,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else { return STATUS_NULL };
    let sender = match host.command_sender() {
        Ok(sender) => sender,
        Err(status) => return status,
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command = HostCommand::SetState { slot, value, reply: reply_sender };
    match send_command(&sender, command, &reply_receiver) {
        Ok(true) => STATUS_OK,
        Ok(false) => STATUS_INVALID_ARGUMENT,
        Err(status) => status,
    }
}

/// Copies every persistent state slot from the live worker in one command.
///
/// # Safety
///
/// `host` must remain live and `output` must point to `capacity` writable `i64` values until the
/// synchronous call returns.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_state_snapshot(
    host: *const SpellwireHost,
    output: *mut i64,
    capacity: usize,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else { return STATUS_NULL };
    let Some(output) = NonNull::new(output) else { return STATUS_INVALID_ARGUMENT };
    let sender = match host.command_sender() {
        Ok(sender) => sender,
        Err(status) => return status,
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command = HostCommand::SnapshotState {
        output: StateSnapshot { output, capacity },
        reply: reply_sender,
    };
    match send_command(&sender, command, &reply_receiver) {
        Ok(status) | Err(status) => status,
    }
}

/// Attaches or detaches the fixed-record shared input ring used by Bun's dynamic lane.
///
/// Passing a null `words` pointer detaches the current ring. A non-null ring must use the layout
/// `[write, read, dropped, closed, ...capacity * 6 event words]`, where capacity is a power of two.
/// The call is synchronous: the buffer must remain live until a successful detach or host stop.
///
/// # Safety
///
/// `host` must remain live. For attachment, `words` must point to `word_len` aligned, writable
/// `i32` words in non-detachable shared storage that obeys the lifetime contract above.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_set_input_ring(
    host: *mut SpellwireHost,
    words: *mut i32,
    word_len: usize,
    capacity: usize,
) -> i32 {
    let Some(host) = (unsafe { host.as_ref() }) else { return STATUS_NULL };
    let ring = if words.is_null() {
        None
    } else {
        // SAFETY: The FFI caller promises the documented buffer validity and lifetime.
        let Some(ring) = (unsafe { DynamicRing::new(words, word_len, capacity) }) else {
            return STATUS_INVALID_ARGUMENT;
        };
        Some(ring)
    };
    let sender = match host.command_sender() {
        Ok(sender) => sender,
        Err(status) => return status,
    };
    let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
    let command = HostCommand::SetInputRing { ring, reply: reply_sender };
    match send_command(&sender, command, &reply_receiver) {
        Ok(()) => STATUS_OK,
        Err(status) => status,
    }
}

/// Copies the latest host error as UTF-8 with a trailing NUL and returns required buffer length.
///
/// Passing null or zero capacity only queries required length.
///
/// # Safety
///
/// When non-null, `buffer` must point to `capacity` writable bytes. `host` must remain live.
#[no_mangle]
pub unsafe extern "C" fn spellwire_host_last_error(
    host: *const SpellwireHost,
    buffer: *mut c_char,
    capacity: usize,
) -> usize {
    let Some(host) = (unsafe { host.as_ref() }) else { return 0 };
    let Ok(error) = host.last_error.lock() else { return 0 };
    let required = error.len().saturating_add(1);
    if buffer.is_null() || capacity == 0 {
        return required;
    }
    let copy_len = error.len().min(capacity.saturating_sub(1));
    // SAFETY: Caller promises `capacity` writable bytes and source/destination do not overlap.
    unsafe {
        ptr::copy_nonoverlapping(error.as_ptr(), buffer.cast(), copy_len);
        buffer.add(copy_len).write(0);
    }
    required
}

#[no_mangle]
pub extern "C" fn spellwire_permission_status() -> u32 {
    platform::permission_status()
}

#[no_mangle]
pub extern "C" fn spellwire_request_permissions() -> u32 {
    platform::request_permissions()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_ring_publishes_fixed_event_records_and_counts_overflow() {
        let capacity = 2;
        let word_len = DYNAMIC_RING_HEADER_WORDS + capacity * DYNAMIC_RING_RECORD_WORDS;
        let mut words: Vec<AtomicI32> = (0..word_len).map(|_| AtomicI32::new(0)).collect();
        // SAFETY: The vector provides the exact aligned storage validated by `DynamicRing::new`
        // and remains allocated for the complete test.
        let ring =
            unsafe { DynamicRing::new(words.as_mut_ptr().cast(), words.len(), capacity).unwrap() };
        let event = InputEvent {
            device: InputDevice::Keyboard,
            code: 0x04,
            edge: Edge::Down,
            source: InputSource::Physical,
        };
        ring.push(event);
        ring.push(event);
        ring.push(event);
        assert_eq!(words[DYNAMIC_RING_WRITE].load(Ordering::Acquire), 2);
        assert_eq!(words[DYNAMIC_RING_DROPPED].load(Ordering::Relaxed), 1);
        assert_eq!(words[DYNAMIC_RING_HEADER_WORDS].load(Ordering::Relaxed), 0);
        assert_eq!(words[DYNAMIC_RING_HEADER_WORDS + 1].load(Ordering::Relaxed), 0x04);
    }
}
