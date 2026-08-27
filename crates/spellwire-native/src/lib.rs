mod host;
pub mod platform;

use core::{ffi::c_void, ptr, ptr::NonNull, slice};
use std::sync::atomic::{AtomicBool, Ordering};

use spellwire_core::{
    Edge, Injector, InputDevice, InputEvent, InputSource, OutputEvent, Program, Runtime,
    RuntimeConfig, VmScratch,
};

const STATUS_ENGINE_BUSY: i32 = -6;

pub type SpellwireOutputCallback = unsafe extern "C" fn(
    context: *mut c_void,
    events: *const SpellwireOutputEvent,
    event_count: usize,
) -> i32;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SpellwireOutputEvent {
    pub kind: u8,
    pub flags: u8,
    pub code: u16,
    pub x: i32,
    pub y: i32,
}

impl SpellwireOutputEvent {
    fn from_core(event: OutputEvent) -> Self {
        match event {
            OutputEvent::Empty => Self { kind: 0, flags: 0, code: 0, x: 0, y: 0 },
            OutputEvent::Key { code, down } => {
                Self { kind: 1, flags: u8::from(down), code, x: 0, y: 0 }
            }
            OutputEvent::MouseButton { button, down } => {
                Self { kind: 2, flags: u8::from(down), code: button as u16, x: 0, y: 0 }
            }
            OutputEvent::MouseMove { dx, dy } => Self { kind: 3, flags: 0, code: 0, x: dx, y: dy },
            OutputEvent::MouseWheel { x, y } => Self { kind: 4, flags: 0, code: 0, x, y },
        }
    }
}

struct CallbackInjector {
    callback: Option<SpellwireOutputCallback>,
    context: *mut c_void,
    converted: [SpellwireOutputEvent; spellwire_core::MAX_OUTPUT_BATCH],
}

impl CallbackInjector {
    const fn new() -> Self {
        Self {
            callback: None,
            context: ptr::null_mut(),
            converted: [SpellwireOutputEvent { kind: 0, flags: 0, code: 0, x: 0, y: 0 };
                spellwire_core::MAX_OUTPUT_BATCH],
        }
    }
}

impl Injector for CallbackInjector {
    type Error = i32;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        let Some(callback) = self.callback else {
            return Ok(());
        };
        for (target, source) in self.converted.iter_mut().zip(events.iter().copied()) {
            *target = SpellwireOutputEvent::from_core(source);
        }
        // SAFETY: The callback and context are provided by the host. The slice points
        // to storage owned by this injector and stays valid for the duration of the call.
        let status = unsafe { callback(self.context, self.converted.as_ptr(), events.len()) };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
}

pub struct SpellwireEngine {
    busy: AtomicBool,
    runtime: Runtime,
    scratch: VmScratch,
    injector: CallbackInjector,
}

impl SpellwireEngine {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ()> {
        let program = Program::decode(bytes).map_err(|_| ())?;
        let runtime = Runtime::new(program, RuntimeConfig::default()).map_err(|_| ())?;
        Ok(Self {
            busy: AtomicBool::new(false),
            runtime,
            scratch: VmScratch::new(),
            injector: CallbackInjector::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineAccessError {
    Null,
    Busy,
}

struct EngineAccess {
    engine: NonNull<SpellwireEngine>,
}

impl EngineAccess {
    unsafe fn acquire(engine: *mut SpellwireEngine) -> Result<Self, EngineAccessError> {
        let engine = NonNull::new(engine).ok_or(EngineAccessError::Null)?;
        // SAFETY: The pointer is non-null. The busy flag is a field disjoint from runtime,
        // scratch, and injector, so callbacks may inspect it while those fields are borrowed.
        let busy = unsafe { &*ptr::addr_of!((*engine.as_ptr()).busy) };
        busy.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map_err(|_| EngineAccessError::Busy)?;
        Ok(Self { engine })
    }

    fn runtime(&self) -> &Runtime {
        // SAFETY: Access acquisition serializes engine operations. This shared reference
        // excludes the busy flag used by reentrant callers to fail without touching runtime.
        unsafe { &*ptr::addr_of!((*self.engine.as_ptr()).runtime) }
    }

    fn runtime_mut(&mut self) -> &mut Runtime {
        // SAFETY: Access acquisition serializes engine operations and `&mut self` prevents
        // another runtime reference from being produced through this guard.
        unsafe { &mut *ptr::addr_of_mut!((*self.engine.as_ptr()).runtime) }
    }

    fn injector_mut(&mut self) -> &mut CallbackInjector {
        // SAFETY: Access acquisition serializes engine operations and `&mut self` prevents
        // another injector reference from being produced through this guard.
        unsafe { &mut *ptr::addr_of_mut!((*self.engine.as_ptr()).injector) }
    }

    fn dispatch_parts(&mut self) -> (&mut Runtime, &mut CallbackInjector, &mut VmScratch) {
        let engine = self.engine.as_ptr();
        // SAFETY: These are disjoint fields. Access acquisition serializes engine operations,
        // and the returned references remain tied to this exclusive guard borrow.
        unsafe {
            (
                &mut *ptr::addr_of_mut!((*engine).runtime),
                &mut *ptr::addr_of_mut!((*engine).injector),
                &mut *ptr::addr_of_mut!((*engine).scratch),
            )
        }
    }
}

impl Drop for EngineAccess {
    fn drop(&mut self) {
        // SAFETY: The engine remains allocated for the lifetime of an access guard. Reentrant
        // free calls observe busy and return without freeing it.
        let busy = unsafe { &*ptr::addr_of!((*self.engine.as_ptr()).busy) };
        busy.store(false, Ordering::Release);
    }
}

#[no_mangle]
pub extern "C" fn spellwire_abi_version() -> u32 {
    5
}

#[no_mangle]
pub extern "C" fn spellwire_capabilities() -> u32 {
    platform::current_capabilities()
}

/// Creates an engine by copying and decoding a complete Spellwire bytecode buffer.
///
/// # Safety
///
/// `bytes` must point to `len` readable bytes for the duration of this call. The returned
/// pointer must be released exactly once with [`spellwire_engine_free`].
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_new(
    bytes: *const u8,
    len: usize,
) -> *mut SpellwireEngine {
    if bytes.is_null() || len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: The caller promises that `bytes` points to `len` readable bytes for
    // the duration of this function. The bytes are decoded and copied into owned data.
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    match SpellwireEngine::from_bytes(bytes) {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(()) => ptr::null_mut(),
    }
}

/// Releases an engine when it is not executing or invoking its output callback.
///
/// A reentrant call from the output callback is ignored so the active dispatch cannot use freed
/// storage. The host must call this function again after dispatch returns.
///
/// # Safety
///
/// `engine` must be null or a live pointer returned by [`spellwire_engine_new`]. It must not have
/// been freed already, and freeing must not race with another thread using the engine.
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_free(engine: *mut SpellwireEngine) {
    let Some(engine) = NonNull::new(engine) else {
        return;
    };
    // SAFETY: The pointer is non-null and the caller promises it points to a live engine.
    let acquired = unsafe { &*ptr::addr_of!((*engine.as_ptr()).busy) }
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_ok();
    if !acquired {
        return;
    }
    // SAFETY: The caller transfers the unique live allocation back to Rust. Setting busy first
    // prevents a callback on this engine from reaching this path while dispatch is active.
    unsafe { drop(Box::from_raw(engine.as_ptr())) };
}

/// Installs or clears the engine's output callback and context pointer.
///
/// Returns `-6` when called reentrantly from an output callback.
///
/// # Safety
///
/// `engine` must point to a live engine. When non-null, `callback` and `context` must remain valid
/// until replaced or the engine is freed. The callback must not call engine APIs on this engine.
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_set_output_callback(
    engine: *mut SpellwireEngine,
    callback: Option<SpellwireOutputCallback>,
    context: *mut c_void,
) -> i32 {
    // SAFETY: The caller promises a live engine pointer; acquisition checks null and reentrancy.
    let mut access = match unsafe { EngineAccess::acquire(engine) } {
        Ok(access) => access,
        Err(EngineAccessError::Null) => return -1,
        Err(EngineAccessError::Busy) => return STATUS_ENGINE_BUSY,
    };
    let injector = access.injector_mut();
    injector.callback = callback;
    injector.context = context;
    0
}

/// Dispatches one explicit input event through matching handlers.
///
/// Returns `-6` when called concurrently or reentrantly on the same engine.
///
/// # Safety
///
/// `engine` must point to a live engine and must not be freed concurrently. The installed output
/// callback must obey its pointer-lifetime contract and must not call engine APIs on this engine.
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_dispatch(
    engine: *mut SpellwireEngine,
    device: u8,
    code: u16,
    edge: u8,
    source: u8,
) -> i32 {
    // SAFETY: The caller promises a live engine pointer; acquisition checks null and reentrancy.
    let mut access = match unsafe { EngineAccess::acquire(engine) } {
        Ok(access) => access,
        Err(EngineAccessError::Null) => return -1,
        Err(EngineAccessError::Busy) => return STATUS_ENGINE_BUSY,
    };
    let Ok(device) = InputDevice::try_from(device) else {
        return -2;
    };
    let Ok(edge) = Edge::try_from(edge) else {
        return -3;
    };
    let Ok(source) = InputSource::try_from(source) else {
        return -4;
    };
    let event = InputEvent { device, code, edge, source };
    let (runtime, injector, scratch) = access.dispatch_parts();
    match runtime.dispatch(event, injector, scratch) {
        Ok(_) => 0,
        Err(_) => -5,
    }
}

/// Reads one persistent state slot into host-provided storage.
///
/// Returns `-6` when called concurrently or reentrantly on the same engine.
///
/// # Safety
///
/// `engine` must point to a live engine, `output` must point to non-overlapping writable `i64`
/// storage, and neither pointer may be freed or invalidated for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_state_get(
    engine: *const SpellwireEngine,
    slot: usize,
    output: *mut i64,
) -> i32 {
    if output.is_null() {
        return -2;
    }
    // SAFETY: The caller promises a live engine pointer; acquisition checks null and reentrancy.
    let access = match unsafe { EngineAccess::acquire(engine.cast_mut()) } {
        Ok(access) => access,
        Err(EngineAccessError::Null) => return -1,
        Err(EngineAccessError::Busy) => return STATUS_ENGINE_BUSY,
    };
    let Some(value) = access.runtime().get_state(slot) else {
        return -3;
    };
    // SAFETY: `output` is non-null and the caller promises writable storage.
    unsafe { output.write(value) };
    0
}

/// Replaces one persistent state slot.
///
/// Returns `-6` when called concurrently or reentrantly on the same engine.
///
/// # Safety
///
/// `engine` must point to a live engine and must not be freed concurrently.
#[no_mangle]
pub unsafe extern "C" fn spellwire_engine_state_set(
    engine: *mut SpellwireEngine,
    slot: usize,
    value: i64,
) -> i32 {
    // SAFETY: The caller promises a live engine pointer; acquisition checks null and reentrancy.
    let mut access = match unsafe { EngineAccess::acquire(engine) } {
        Ok(access) => access,
        Err(EngineAccessError::Null) => return -1,
        Err(EngineAccessError::Busy) => return STATUS_ENGINE_BUSY,
    };
    if access.runtime_mut().set_state(slot, value) {
        0
    } else {
        -2
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI32, Ordering};

    use spellwire_core::{
        key, Edge, Handler, InputDevice, Instruction, Opcode, Program, SourceFilter, Trigger,
    };

    use super::*;

    static REENTRANT_STATUS: AtomicI32 = AtomicI32::new(0);

    fn output_engine() -> *mut SpellwireEngine {
        let program = Program {
            initial_state: Vec::new().into_boxed_slice(),
            handlers: vec![Handler {
                trigger: Trigger {
                    device: InputDevice::Keyboard,
                    code: key::Q,
                    edge: Edge::Down,
                    source: SourceFilter::Physical,
                    flags: 0,
                    modifiers: 0,
                    gate: spellwire_core::NO_STATE_GATE,
                },
                entry: 0,
            }]
            .into_boxed_slice(),
            code: vec![
                Instruction::new(Opcode::KeyDown).with_a(key::E),
                Instruction::new(Opcode::Halt),
            ]
            .into_boxed_slice(),
            local_count: 0,
            stack_limit: 8,
            instruction_budget: 100,
        };
        let engine = SpellwireEngine {
            busy: AtomicBool::new(false),
            runtime: Runtime::new(program, RuntimeConfig::default()).unwrap(),
            scratch: VmScratch::new(),
            injector: CallbackInjector::new(),
        };
        Box::into_raw(Box::new(engine))
    }

    unsafe extern "C" fn reentrant_dispatch(
        context: *mut c_void,
        _events: *const SpellwireOutputEvent,
        _event_count: usize,
    ) -> i32 {
        // SAFETY: The test context is the live engine pointer. Reentrant access must be rejected
        // before any runtime field is borrowed again.
        let status = unsafe {
            spellwire_engine_dispatch(
                context.cast(),
                InputDevice::Keyboard as u8,
                key::Q,
                Edge::Down as u8,
                InputSource::Physical as u8,
            )
        };
        REENTRANT_STATUS.store(status, Ordering::Relaxed);
        0
    }

    unsafe extern "C" fn reentrant_free(
        context: *mut c_void,
        _events: *const SpellwireOutputEvent,
        _event_count: usize,
    ) -> i32 {
        // SAFETY: The test intentionally attempts a reentrant free. The engine guard must make
        // this a no-op so the active dispatch can finish safely.
        unsafe { spellwire_engine_free(context.cast()) };
        0
    }

    #[test]
    fn reports_owned_host_abi_capabilities() {
        assert_eq!(spellwire_abi_version(), 5);
        assert_eq!(spellwire_capabilities(), platform::current_capabilities());
    }

    #[test]
    fn rejects_reentrant_dispatch_from_output_callback() {
        let engine = output_engine();
        // SAFETY: The engine remains live for this test and is freed exactly once below.
        unsafe {
            assert_eq!(
                spellwire_engine_set_output_callback(
                    engine,
                    Some(reentrant_dispatch),
                    engine.cast(),
                ),
                0
            );
            assert_eq!(
                spellwire_engine_dispatch(
                    engine,
                    InputDevice::Keyboard as u8,
                    key::Q,
                    Edge::Down as u8,
                    InputSource::Physical as u8,
                ),
                0
            );
            assert_eq!(REENTRANT_STATUS.load(Ordering::Relaxed), STATUS_ENGINE_BUSY);
            spellwire_engine_free(engine);
        }
    }

    #[test]
    fn ignores_reentrant_free_until_dispatch_returns() {
        let engine = output_engine();
        // SAFETY: The callback's free attempt is guarded. The engine remains live until the
        // explicit free after dispatch returns.
        unsafe {
            assert_eq!(
                spellwire_engine_set_output_callback(engine, Some(reentrant_free), engine.cast()),
                0
            );
            assert_eq!(
                spellwire_engine_dispatch(
                    engine,
                    InputDevice::Keyboard as u8,
                    key::Q,
                    Edge::Down as u8,
                    InputSource::Physical as u8,
                ),
                0
            );
            assert_eq!(spellwire_engine_state_set(engine, 0, 1), -2);
            spellwire_engine_free(engine);
        }
    }
}
