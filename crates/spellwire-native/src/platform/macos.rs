use core::{ffi::c_void, ptr};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

use spellwire_core::{
    key, Edge, Injector, InputDevice, InputEvent, InputSource, InputState, MouseButton, OutputEvent,
};

use super::{
    InputPolicy, InputSender, Observer, PlatformError, PlatformInjector, PERMISSION_INJECT,
    PERMISSION_OBSERVE,
};

const SPELLWIRE_EVENT_TAG: i64 = 0x5350_454c_4c57_4952;
const KEYBOARD_KEYCODE_FIELD: u32 = 9;
const MOUSE_BUTTON_NUMBER_FIELD: u32 = 3;
const EVENT_SOURCE_USER_DATA_FIELD: u32 = 42;

const EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const EVENT_LEFT_MOUSE_UP: u32 = 2;
const EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const EVENT_RIGHT_MOUSE_UP: u32 = 4;
const EVENT_MOUSE_MOVED: u32 = 5;
const EVENT_KEY_DOWN: u32 = 10;
const EVENT_KEY_UP: u32 = 11;
const EVENT_FLAGS_CHANGED: u32 = 12;
const EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const EVENT_OTHER_MOUSE_UP: u32 = 26;
const EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xffff_fffe;
const EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xffff_ffff;

const SESSION_EVENT_TAP: u32 = 1;
const HEAD_INSERT_EVENT_TAP: u32 = 0;
const DEFAULT_EVENT_TAP: u32 = 0;
const HID_EVENT_TAP: u32 = 0;
const PRIVATE_EVENT_SOURCE: i32 = -1;
const PIXEL_SCROLL_UNIT: u32 = 0;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

type CFTypeRef = *const c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFStringRef = *const c_void;
type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CGEventTapCallback =
    unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventCreateMouseEvent(
        source: CGEventSourceRef,
        mouse_type: u32,
        mouse_cursor_position: CGPoint,
        mouse_button: u32,
    ) -> CGEventRef;
    fn CGEventCreateScrollWheelEvent2(
        source: CGEventSourceRef,
        units: u32,
        wheel_count: u32,
        wheel_1: i32,
        wheel_2: i32,
        wheel_3: i32,
    ) -> CGEventRef;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGEventSourceCreate(state_id: i32) -> CGEventSourceRef;
    fn CGEventSourceSetUserData(source: CGEventSourceRef, user_data: i64);
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn CGPreflightPostEventAccess() -> bool;
    fn CGRequestPostEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(run_loop: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;
    fn CFRelease(value: CFTypeRef);
}

struct MacInjector {
    source: CGEventSourceRef,
}

// SAFETY: The source is exclusively owned and used by the host worker thread after construction.
unsafe impl Send for MacInjector {}

impl MacInjector {
    fn post(event: CGEventRef) -> Result<(), PlatformError> {
        if event.is_null() {
            return Err(PlatformError::Initialization("CoreGraphics could not create an event"));
        }
        // SAFETY: `event` is a live +1 CoreGraphics object created above. It is tagged before
        // posting so the observer can classify it as synthetic, then released exactly once.
        unsafe {
            CGEventSetIntegerValueField(event, EVENT_SOURCE_USER_DATA_FIELD, SPELLWIRE_EVENT_TAG);
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event.cast_const());
        }
        Ok(())
    }

    fn cursor_location() -> Result<CGPoint, PlatformError> {
        // SAFETY: A null source asks CoreGraphics for a generic event containing current cursor
        // state. The returned +1 object is released after copying the CGPoint value.
        let event = unsafe { CGEventCreate(ptr::null_mut()) };
        if event.is_null() {
            return Err(PlatformError::Initialization(
                "CoreGraphics could not read the cursor position",
            ));
        }
        // SAFETY: `event` is valid until the following release.
        let location = unsafe { CGEventGetLocation(event) };
        // SAFETY: `event` is a live +1 CoreGraphics object.
        unsafe { CFRelease(event.cast_const()) };
        Ok(location)
    }
}

impl Drop for MacInjector {
    fn drop(&mut self) {
        if !self.source.is_null() {
            // SAFETY: The injector owns the +1 event source and releases it exactly once.
            unsafe { CFRelease(self.source.cast_const()) };
        }
    }
}

impl Injector for MacInjector {
    type Error = PlatformError;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        for event in events {
            match *event {
                OutputEvent::Empty => {}
                OutputEvent::Key { code, down } => {
                    let keycode =
                        hid_to_mac_keycode(code).ok_or(PlatformError::UnsupportedKey(code))?;
                    // SAFETY: The source is live and the keycode comes from the audited map.
                    let event = unsafe { CGEventCreateKeyboardEvent(self.source, keycode, down) };
                    Self::post(event)?;
                }
                OutputEvent::MouseButton { button, down } => {
                    let location = Self::cursor_location()?;
                    let (button, event_type) = mouse_button_event(button, down);
                    // SAFETY: The source is live and arguments use CoreGraphics enum values.
                    let event = unsafe {
                        CGEventCreateMouseEvent(self.source, event_type, location, button)
                    };
                    Self::post(event)?;
                }
                OutputEvent::MouseMove { dx, dy } => {
                    let current = Self::cursor_location()?;
                    let location =
                        CGPoint { x: current.x + f64::from(dx), y: current.y + f64::from(dy) };
                    // SAFETY: The source is live and integer deltas convert exactly to f64.
                    let event = unsafe {
                        CGEventCreateMouseEvent(self.source, EVENT_MOUSE_MOVED, location, 0)
                    };
                    Self::post(event)?;
                }
                OutputEvent::MouseWheel { x, y } => {
                    // SAFETY: The source is live; CoreGraphics accepts two signed pixel axes.
                    let event = unsafe {
                        CGEventCreateScrollWheelEvent2(self.source, PIXEL_SCROLL_UNIT, 2, y, x, 0)
                    };
                    Self::post(event)?;
                }
            }
        }
        Ok(())
    }
}

pub fn create_injector() -> Result<PlatformInjector, PlatformError> {
    // SAFETY: This is a read-only OS permission query.
    if !unsafe { CGPreflightPostEventAccess() } {
        return Err(PlatformError::PermissionDenied("macOS Accessibility"));
    }
    // SAFETY: A private source is process-owned and safe to create off the main thread.
    let source = unsafe { CGEventSourceCreate(PRIVATE_EVENT_SOURCE) };
    if source.is_null() {
        return Err(PlatformError::Initialization(
            "CoreGraphics could not create the private event source",
        ));
    }
    // SAFETY: The source is live and owned by the injector.
    unsafe { CGEventSourceSetUserData(source, SPELLWIRE_EVENT_TAG) };
    Ok(Box::new(MacInjector { source }))
}

struct ObserverContext {
    sender: InputSender,
    policy: Arc<InputPolicy>,
    input_state: InputState,
    consumed_state: InputState,
    tap: CFMachPortRef,
}

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if user_info.is_null() || event.is_null() {
        return event;
    }
    // SAFETY: `user_info` points to the boxed ObserverContext owned by the observer thread for
    // the complete lifetime of the tap callback.
    let context = unsafe { &mut *user_info.cast::<ObserverContext>() };
    if event_type == EVENT_TAP_DISABLED_BY_TIMEOUT || event_type == EVENT_TAP_DISABLED_BY_USER_INPUT
    {
        if !context.tap.is_null() {
            // SAFETY: The tap is live while the callback is installed.
            unsafe { CGEventTapEnable(context.tap, true) };
        }
        return event;
    }

    // SAFETY: CoreGraphics supplied a live event for the duration of this callback.
    let tag = unsafe { CGEventGetIntegerValueField(event, EVENT_SOURCE_USER_DATA_FIELD) };
    let source =
        if tag == SPELLWIRE_EVENT_TAG { InputSource::Synthetic } else { InputSource::Physical };

    let translated = match event_type {
        EVENT_KEY_DOWN | EVENT_KEY_UP | EVENT_FLAGS_CHANGED => {
            // SAFETY: The field is defined for keyboard and flags-changed events.
            let raw_keycode = unsafe { CGEventGetIntegerValueField(event, KEYBOARD_KEYCODE_FIELD) };
            let keycode = u16::try_from(raw_keycode).ok();
            let code = keycode.and_then(mac_keycode_to_hid);
            let edge = if event_type == EVENT_FLAGS_CHANGED {
                let Some(keycode) = keycode else { return event };
                // SAFETY: Combined-session key state is a read-only OS query for this keycode.
                if unsafe { CGEventSourceKeyState(0, keycode) } {
                    Edge::Down
                } else {
                    Edge::Up
                }
            } else if event_type == EVENT_KEY_DOWN {
                Edge::Down
            } else {
                Edge::Up
            };
            code.map(|code| InputEvent { device: InputDevice::Keyboard, code, edge, source })
        }
        EVENT_LEFT_MOUSE_DOWN | EVENT_LEFT_MOUSE_UP => Some(InputEvent {
            device: InputDevice::MouseButton,
            code: MouseButton::Left as u16,
            edge: if event_type == EVENT_LEFT_MOUSE_DOWN { Edge::Down } else { Edge::Up },
            source,
        }),
        EVENT_RIGHT_MOUSE_DOWN | EVENT_RIGHT_MOUSE_UP => Some(InputEvent {
            device: InputDevice::MouseButton,
            code: MouseButton::Right as u16,
            edge: if event_type == EVENT_RIGHT_MOUSE_DOWN { Edge::Down } else { Edge::Up },
            source,
        }),
        EVENT_OTHER_MOUSE_DOWN | EVENT_OTHER_MOUSE_UP => {
            // SAFETY: The field is defined for other-mouse-button events.
            let raw_button =
                unsafe { CGEventGetIntegerValueField(event, MOUSE_BUTTON_NUMBER_FIELD) };
            let button = match raw_button {
                2 => Some(MouseButton::Middle),
                3 => Some(MouseButton::Back),
                4 => Some(MouseButton::Forward),
                _ => None,
            };
            button.map(|button| InputEvent {
                device: InputDevice::MouseButton,
                code: button as u16,
                edge: if event_type == EVENT_OTHER_MOUSE_DOWN { Edge::Down } else { Edge::Up },
                source,
            })
        }
        _ => None,
    };

    if let Some(input) = translated {
        let mut consume = false;
        for normalized in normalize_input(event_type, input).into_iter().flatten() {
            consume |= process_observed(context, normalized);
        }
        if consume {
            return ptr::null_mut();
        }
    }
    event
}

fn normalize_input(event_type: u32, input: InputEvent) -> [Option<InputEvent>; 2] {
    if event_type == EVENT_FLAGS_CHANGED
        && input.device == InputDevice::Keyboard
        && input.code == key::CAPS_LOCK
    {
        [
            Some(InputEvent { edge: Edge::Down, ..input }),
            Some(InputEvent { edge: Edge::Up, ..input }),
        ]
    } else {
        [Some(input), None]
    }
}

fn process_observed(context: &mut ObserverContext, input: InputEvent) -> bool {
    let modifiers = context.input_state.modifiers_for_event(input);
    let repeated = input.edge == Edge::Down
        && context.input_state.held_for_source(input.device, input.code, input.source);
    let paired = context.consumed_state.held_for_source(input.device, input.code, input.source);
    let consume = paired || context.policy.should_consume(input, modifiers, repeated);
    context.input_state.apply(input);
    if context.sender.try_send(input) {
        if consume || input.edge == Edge::Up {
            context.consumed_state.apply(input);
        }
        consume
    } else {
        if input.edge == Edge::Up {
            context.consumed_state.apply(input);
        }
        false
    }
}

pub fn start_observer(
    sender: InputSender,
    policy: Arc<InputPolicy>,
) -> Result<Observer, PlatformError> {
    // SAFETY: This is a read-only OS permission query.
    if !unsafe { CGPreflightListenEventAccess() } {
        return Err(PlatformError::PermissionDenied("macOS Input Monitoring"));
    }

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (setup_sender, setup_receiver) = sync_channel(1);
    let join = thread::Builder::new()
        .name("spellwire-macos-observer".into())
        .spawn(move || run_observer(sender, policy, worker_stop, setup_sender))?;
    match setup_receiver.recv() {
        Ok(Ok(())) => Ok(Observer::new(stop, join)),
        Ok(Err(error)) => {
            let _ = join.join();
            Err(error)
        }
        Err(_) => {
            let _ = join.join();
            Err(PlatformError::Initialization(
                "macOS observer exited before initialization completed",
            ))
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_observer(
    sender: InputSender,
    policy: Arc<InputPolicy>,
    stop: Arc<AtomicBool>,
    setup: SyncSender<Result<(), PlatformError>>,
) -> Result<(), PlatformError> {
    let mut context = Box::new(ObserverContext {
        sender,
        policy,
        input_state: InputState::new(),
        consumed_state: InputState::new(),
        tap: ptr::null_mut(),
    });
    let mask = [
        EVENT_LEFT_MOUSE_DOWN,
        EVENT_LEFT_MOUSE_UP,
        EVENT_RIGHT_MOUSE_DOWN,
        EVENT_RIGHT_MOUSE_UP,
        EVENT_KEY_DOWN,
        EVENT_KEY_UP,
        EVENT_FLAGS_CHANGED,
        EVENT_OTHER_MOUSE_DOWN,
        EVENT_OTHER_MOUSE_UP,
    ]
    .into_iter()
    .fold(0_u64, |mask, event_type| mask | (1_u64 << event_type));

    // SAFETY: Context storage remains in the Box until after the run-loop source and tap are
    // released. The callback signature and event mask match CoreGraphics.
    let tap = unsafe {
        CGEventTapCreate(
            SESSION_EVENT_TAP,
            HEAD_INSERT_EVENT_TAP,
            DEFAULT_EVENT_TAP,
            mask,
            event_tap_callback,
            ptr::addr_of_mut!(*context).cast(),
        )
    };
    if tap.is_null() {
        let message = "CoreGraphics could not create the session event tap";
        let _ = setup.send(Err(PlatformError::Initialization(message)));
        return Err(PlatformError::Initialization(message));
    }
    context.tap = tap;
    // SAFETY: `tap` is live and null allocator selects the default allocator.
    let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
    if source.is_null() {
        // SAFETY: `tap` is a live +1 object.
        unsafe { CFRelease(tap.cast_const()) };
        let message = "CoreFoundation could not create the event-tap run-loop source";
        let _ = setup.send(Err(PlatformError::Initialization(message)));
        return Err(PlatformError::Initialization(message));
    }

    // SAFETY: Source and current run loop are live on this worker thread; the default-mode
    // constant is provided by CoreFoundation.
    unsafe {
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
    }
    let _ = setup.send(Ok(()));
    while !stop.load(Ordering::Acquire) {
        // SAFETY: Runs this thread's loop for a bounded interval so stop remains responsive.
        unsafe {
            CFRunLoopRunInMode(
                kCFRunLoopDefaultMode,
                Duration::from_millis(25).as_secs_f64(),
                false,
            );
        }
    }

    // SAFETY: The source and tap are live +1 CoreFoundation objects and callbacks cannot run
    // after this worker stops servicing the loop.
    unsafe {
        CGEventTapEnable(tap, false);
        CFRelease(source.cast_const());
        CFRelease(tap.cast_const());
    }
    Ok(())
}

#[must_use]
pub fn permission_status() -> u32 {
    let mut status = 0;
    // SAFETY: Both calls are read-only OS permission queries.
    unsafe {
        if CGPreflightListenEventAccess() {
            status |= PERMISSION_OBSERVE;
        }
        if CGPreflightPostEventAccess() {
            status |= PERMISSION_INJECT;
        }
    }
    status
}

#[must_use]
pub fn request_permissions() -> u32 {
    // SAFETY: These documented APIs may show the corresponding macOS permission prompts.
    unsafe {
        let _ = CGRequestListenEventAccess();
        let _ = CGRequestPostEventAccess();
    }
    permission_status()
}

const fn mouse_button_event(button: MouseButton, down: bool) -> (u32, u32) {
    match (button, down) {
        (MouseButton::Left, true) => (0, EVENT_LEFT_MOUSE_DOWN),
        (MouseButton::Left, false) => (0, EVENT_LEFT_MOUSE_UP),
        (MouseButton::Right, true) => (1, EVENT_RIGHT_MOUSE_DOWN),
        (MouseButton::Right, false) => (1, EVENT_RIGHT_MOUSE_UP),
        (MouseButton::Middle, true) => (2, EVENT_OTHER_MOUSE_DOWN),
        (MouseButton::Middle, false) => (2, EVENT_OTHER_MOUSE_UP),
        (MouseButton::Back, true) => (3, EVENT_OTHER_MOUSE_DOWN),
        (MouseButton::Back, false) => (3, EVENT_OTHER_MOUSE_UP),
        (MouseButton::Forward, true) => (4, EVENT_OTHER_MOUSE_DOWN),
        (MouseButton::Forward, false) => (4, EVENT_OTHER_MOUSE_UP),
    }
}

#[allow(clippy::too_many_lines)]
const fn hid_to_mac_keycode(code: u16) -> Option<u16> {
    Some(match code {
        0x04 => 0,
        0x05 => 11,
        0x06 => 8,
        0x07 => 2,
        0x08 => 14,
        0x09 => 3,
        0x0a => 5,
        0x0b => 4,
        0x0c => 34,
        0x0d => 38,
        0x0e => 40,
        0x0f => 37,
        0x10 => 46,
        0x11 => 45,
        0x12 => 31,
        0x13 => 35,
        0x14 => 12,
        0x15 => 15,
        0x16 => 1,
        0x17 => 17,
        0x18 => 32,
        0x19 => 9,
        0x1a => 13,
        0x1b => 7,
        0x1c => 16,
        0x1d => 6,
        0x1e => 18,
        0x1f => 19,
        0x20 => 20,
        0x21 => 21,
        0x22 => 23,
        0x23 => 22,
        0x24 => 26,
        0x25 => 28,
        0x26 => 25,
        0x27 => 29,
        0x28 => 36,
        0x29 => 53,
        0x2a => 51,
        0x2b => 48,
        0x2c => 49,
        0x2d => 27,
        0x2e => 24,
        0x2f => 33,
        0x30 => 30,
        0x31 => 42,
        0x32 | 0x64 => 10,
        0x33 => 41,
        0x34 => 39,
        0x35 => 50,
        0x36 => 43,
        0x37 => 47,
        0x38 => 44,
        0x39 => 57,
        0x3a => 122,
        0x3b => 120,
        0x3c => 99,
        0x3d => 118,
        0x3e => 96,
        0x3f => 97,
        0x40 => 98,
        0x41 => 100,
        0x42 => 101,
        0x43 => 109,
        0x44 => 103,
        0x45 => 111,
        0x49 => 114,
        0x4a => 115,
        0x4b => 116,
        0x4c => 117,
        0x4d => 119,
        0x4e => 121,
        0x4f => 124,
        0x50 => 123,
        0x51 => 125,
        0x52 => 126,
        0x53 => 71,
        0x54 => 75,
        0x55 => 67,
        0x56 => 78,
        0x57 => 69,
        0x58 => 76,
        0x59 => 83,
        0x5a => 84,
        0x5b => 85,
        0x5c => 86,
        0x5d => 87,
        0x5e => 88,
        0x5f => 89,
        0x60 => 91,
        0x61 => 92,
        0x62 => 82,
        0x63 => 65,
        0x67 => 81,
        0x68 => 105,
        0x69 => 107,
        0x6a => 113,
        0x6b => 106,
        0x6c => 64,
        0x6d => 79,
        0x6e => 80,
        0x6f => 90,
        0x7f => 74,
        0x80 => 72,
        0x81 => 73,
        0x87 => 94,
        0x89 => 93,
        0x90 => 104,
        0x91 => 102,
        0xe0 => 59,
        0xe1 => 56,
        0xe2 => 58,
        0xe3 => 55,
        0xe4 => 62,
        0xe5 => 60,
        0xe6 => 61,
        0xe7 => 54,
        _ => return None,
    })
}

const fn mac_keycode_to_hid(keycode: u16) -> Option<u16> {
    let mut usage = 0_u16;
    while usage <= 0xe7 {
        if let Some(candidate) = hid_to_mac_keycode(usage) {
            if candidate == keycode {
                return Some(usage);
            }
        }
        usage += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_key_map_round_trips() {
        for usage in 0_u16..=0xe7 {
            let Some(keycode) = hid_to_mac_keycode(usage) else { continue };
            let canonical = mac_keycode_to_hid(keycode).unwrap();
            assert_eq!(hid_to_mac_keycode(canonical), Some(keycode));
        }
    }

    #[test]
    fn maps_letters_navigation_keypad_international_and_modifiers() {
        assert_eq!(hid_to_mac_keycode(0x04), Some(0));
        assert_eq!(hid_to_mac_keycode(0x52), Some(126));
        assert_eq!(hid_to_mac_keycode(0x58), Some(76));
        assert_eq!(hid_to_mac_keycode(0x87), Some(94));
        assert_eq!(hid_to_mac_keycode(0xe7), Some(54));
        assert_eq!(hid_to_mac_keycode(0xff), None);
    }

    #[test]
    fn normalizes_caps_lock_flags_change_to_one_press_pulse() {
        let input = InputEvent {
            device: InputDevice::Keyboard,
            code: key::CAPS_LOCK,
            edge: Edge::Up,
            source: InputSource::Physical,
        };
        let normalized = normalize_input(EVENT_FLAGS_CHANGED, input);

        assert_eq!(normalized[0].map(|event| event.edge), Some(Edge::Down));
        assert_eq!(normalized[1].map(|event| event.edge), Some(Edge::Up));
    }
}
