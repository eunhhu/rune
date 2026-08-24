mod platform;

use core::{ffi::c_void, ptr, slice};

use rune_core::{
    Edge, Injector, InputDevice, InputEvent, InputSource, OutputEvent, Program, Runtime,
    RuntimeConfig, VmScratch,
};

pub type RuneOutputCallback = unsafe extern "C" fn(
    context: *mut c_void,
    events: *const RuneOutputEvent,
    event_count: usize,
) -> i32;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RuneOutputEvent {
    pub kind: u8,
    pub flags: u8,
    pub code: u16,
    pub x: i32,
    pub y: i32,
}

impl RuneOutputEvent {
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
    callback: Option<RuneOutputCallback>,
    context: *mut c_void,
    converted: [RuneOutputEvent; rune_core::MAX_OUTPUT_BATCH],
}

impl CallbackInjector {
    const fn new() -> Self {
        Self {
            callback: None,
            context: ptr::null_mut(),
            converted: [RuneOutputEvent { kind: 0, flags: 0, code: 0, x: 0, y: 0 };
                rune_core::MAX_OUTPUT_BATCH],
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
            *target = RuneOutputEvent::from_core(source);
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

pub struct RuneEngine {
    runtime: Runtime,
    scratch: VmScratch,
    injector: CallbackInjector,
}

impl RuneEngine {
    fn from_bytes(bytes: &[u8]) -> Result<Self, ()> {
        let program = Program::decode(bytes).map_err(|_| ())?;
        let runtime = Runtime::new(program, RuntimeConfig::default()).map_err(|_| ())?;
        Ok(Self { runtime, scratch: VmScratch::new(), injector: CallbackInjector::new() })
    }
}

#[no_mangle]
pub extern "C" fn rune_abi_version() -> u32 {
    2
}

#[no_mangle]
pub extern "C" fn rune_capabilities() -> u32 {
    platform::current_capabilities()
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_new(bytes: *const u8, len: usize) -> *mut RuneEngine {
    if bytes.is_null() || len == 0 {
        return ptr::null_mut();
    }
    // SAFETY: The caller promises that `bytes` points to `len` readable bytes for
    // the duration of this function. The bytes are decoded and copied into owned data.
    let bytes = unsafe { slice::from_raw_parts(bytes, len) };
    match RuneEngine::from_bytes(bytes) {
        Ok(engine) => Box::into_raw(Box::new(engine)),
        Err(()) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_free(engine: *mut RuneEngine) {
    if !engine.is_null() {
        // SAFETY: A non-null pointer must have been returned by `rune_engine_new`
        // and must be freed exactly once.
        unsafe { drop(Box::from_raw(engine)) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_set_output_callback(
    engine: *mut RuneEngine,
    callback: Option<RuneOutputCallback>,
    context: *mut c_void,
) -> i32 {
    // SAFETY: The pointer is checked and only dereferenced for this call.
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return -1;
    };
    engine.injector.callback = callback;
    engine.injector.context = context;
    0
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_dispatch(
    engine: *mut RuneEngine,
    device: u8,
    code: u16,
    edge: u8,
    source: u8,
) -> i32 {
    // SAFETY: The pointer is checked and only dereferenced for this call. RuneEngine
    // is single-owner: the host must not call this concurrently.
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return -1;
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
    match engine.runtime.dispatch(event, &mut engine.injector, &mut engine.scratch) {
        Ok(_) => 0,
        Err(_) => -5,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_state_get(
    engine: *const RuneEngine,
    slot: usize,
    output: *mut i64,
) -> i32 {
    if output.is_null() {
        return -2;
    }
    // SAFETY: Both pointers are checked before dereferencing.
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return -1;
    };
    let Some(value) = engine.runtime.get_state(slot) else {
        return -3;
    };
    // SAFETY: `output` is non-null and the caller promises writable storage.
    unsafe { output.write(value) };
    0
}

#[no_mangle]
pub unsafe extern "C" fn rune_engine_state_set(
    engine: *mut RuneEngine,
    slot: usize,
    value: i64,
) -> i32 {
    // SAFETY: The pointer is checked and only dereferenced for this call.
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return -1;
    };
    if engine.runtime.set_state(slot, value) {
        0
    } else {
        -2
    }
}
