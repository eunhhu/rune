use core::{mem::size_of, ptr};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        mpsc::{sync_channel, SyncSender, TrySendError},
        Arc,
    },
    thread,
};

use spellwire_core::{
    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent,
};
use windows_sys::Win32::{
    System::{
        LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        },
        Threading::GetCurrentThreadId,
    },
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
            KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_HWHEEL,
            MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
            MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
            MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
            TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
            LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_QUIT, WM_RBUTTONDOWN,
            WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

use super::{Observer, PlatformError, PlatformInjector, PERMISSION_INJECT, PERMISSION_OBSERVE};

const SPELLWIRE_EVENT_TAG: usize = 0x5350_454c_4c57_4952;
const HC_ACTION: i32 = 0;
const XBUTTON1: u32 = 1;
const XBUTTON2: u32 = 2;

static OBSERVER_SENDER: AtomicPtr<SyncSender<InputEvent>> = AtomicPtr::new(ptr::null_mut());

struct WindowsInjector {
    inputs: Vec<INPUT>,
}

impl Injector for WindowsInjector {
    type Error = PlatformError;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        self.inputs.clear();
        for event in events {
            match *event {
                OutputEvent::Empty => {}
                OutputEvent::Key { code, down } => {
                    let (scan, extended) =
                        hid_to_scan(code).ok_or(PlatformError::UnsupportedKey(code))?;
                    let mut flags = KEYEVENTF_SCANCODE;
                    if extended {
                        flags |= KEYEVENTF_EXTENDEDKEY;
                    }
                    if !down {
                        flags |= KEYEVENTF_KEYUP;
                    }
                    self.inputs.push(INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: 0,
                                wScan: scan,
                                dwFlags: flags,
                                time: 0,
                                dwExtraInfo: SPELLWIRE_EVENT_TAG,
                            },
                        },
                    });
                }
                OutputEvent::MouseButton { button, down } => {
                    let (flags, data) = mouse_button_input(button, down);
                    self.inputs.push(mouse_input(0, 0, data, flags));
                }
                OutputEvent::MouseMove { dx, dy } => {
                    self.inputs.push(mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE));
                }
                OutputEvent::MouseWheel { x, y } => {
                    if y != 0 {
                        self.inputs.push(mouse_input(
                            0,
                            0,
                            u32::from_ne_bytes(y.to_ne_bytes()),
                            MOUSEEVENTF_WHEEL,
                        ));
                    }
                    if x != 0 {
                        self.inputs.push(mouse_input(
                            0,
                            0,
                            u32::from_ne_bytes(x.to_ne_bytes()),
                            MOUSEEVENTF_HWHEEL,
                        ));
                    }
                }
            }
        }
        if self.inputs.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(self.inputs.len())
            .map_err(|_| PlatformError::Initialization("Windows input batch is too large"))?;
        let input_size = i32::try_from(size_of::<INPUT>())
            .map_err(|_| PlatformError::Initialization("Windows INPUT size does not fit i32"))?;
        // SAFETY: `inputs` is a contiguous initialized INPUT array and count/size match it.
        let sent = unsafe { SendInput(count, self.inputs.as_ptr(), input_size) };
        if sent != count {
            return Err(PlatformError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

const fn mouse_input(dx: i32, dy: i32, data: u32, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SPELLWIRE_EVENT_TAG,
            },
        },
    }
}

const fn mouse_button_input(button: MouseButton, down: bool) -> (u32, u32) {
    match (button, down) {
        (MouseButton::Left, true) => (MOUSEEVENTF_LEFTDOWN, 0),
        (MouseButton::Left, false) => (MOUSEEVENTF_LEFTUP, 0),
        (MouseButton::Right, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (MouseButton::Right, false) => (MOUSEEVENTF_RIGHTUP, 0),
        (MouseButton::Middle, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (MouseButton::Middle, false) => (MOUSEEVENTF_MIDDLEUP, 0),
        (MouseButton::Back, true) => (MOUSEEVENTF_XDOWN, XBUTTON1),
        (MouseButton::Back, false) => (MOUSEEVENTF_XUP, XBUTTON1),
        (MouseButton::Forward, true) => (MOUSEEVENTF_XDOWN, XBUTTON2),
        (MouseButton::Forward, false) => (MOUSEEVENTF_XUP, XBUTTON2),
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn create_injector() -> Result<PlatformInjector, PlatformError> {
    Ok(Box::new(WindowsInjector {
        inputs: Vec::with_capacity(spellwire_core::MAX_OUTPUT_BATCH * 2),
    }))
}

pub fn start_observer(sender: SyncSender<InputEvent>) -> Result<Observer, PlatformError> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (setup_sender, setup_receiver) = sync_channel(1);
    let join = thread::Builder::new()
        .name("spellwire-windows-observer".into())
        .spawn(move || run_observer(sender, worker_stop, setup_sender))?;
    match setup_receiver.recv() {
        Ok(Ok(thread_id)) => Ok(Observer::new_with_wake(
            stop,
            join,
            Box::new(move || {
                // SAFETY: Posting WM_QUIT wakes the observer's message queue for clean shutdown.
                unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
            }),
        )),
        Ok(Err(message)) => {
            let _ = join.join();
            Err(PlatformError::Initialization(message))
        }
        Err(_) => {
            let _ = join.join();
            Err(PlatformError::Initialization(
                "Windows observer exited before initialization completed",
            ))
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_observer(
    sender: SyncSender<InputEvent>,
    stop: Arc<AtomicBool>,
    setup: SyncSender<Result<u32, &'static str>>,
) -> Result<(), PlatformError> {
    let sender = Box::into_raw(Box::new(sender));
    if OBSERVER_SENDER
        .compare_exchange(ptr::null_mut(), sender, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        // SAFETY: This thread still uniquely owns the unpublished sender allocation.
        unsafe { drop(Box::from_raw(sender)) };
        let message = "only one Windows observer may run per process";
        let _ = setup.send(Err(message));
        return Err(PlatformError::Initialization(message));
    }

    let mut module = ptr::null_mut();
    // SAFETY: FROM_ADDRESS interprets the callback address as an address inside the containing
    // module rather than as a string. UNCHANGED_REFCOUNT avoids creating an unmatched reference.
    let module_status = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            (keyboard_hook as *const ()).cast(),
            ptr::addr_of_mut!(module),
        )
    };
    if module_status == 0 {
        clear_sender(sender);
        let message = "GetModuleHandleExW failed for the Spellwire native library";
        let _ = setup.send(Err(message));
        return Err(PlatformError::Io(std::io::Error::last_os_error()));
    }

    // SAFETY: `module` contains the callback and thread id zero requests a global low-level hook.
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0) };
    if keyboard.is_null() {
        clear_sender(sender);
        let message = "SetWindowsHookExW failed for the low-level keyboard hook";
        let _ = setup.send(Err(message));
        return Err(PlatformError::Io(std::io::Error::last_os_error()));
    }
    // SAFETY: Same contract as the keyboard hook above; both callbacks are in this module.
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0) };
    if mouse.is_null() {
        // SAFETY: `keyboard` is a live hook owned by this thread.
        unsafe { UnhookWindowsHookEx(keyboard) };
        clear_sender(sender);
        let message = "SetWindowsHookExW failed for the low-level mouse hook";
        let _ = setup.send(Err(message));
        return Err(PlatformError::Io(std::io::Error::last_os_error()));
    }

    // SAFETY: Returns the current observer thread identifier.
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = setup.send(Ok(thread_id));
    // SAFETY: MSG is a plain C record for GetMessageW to initialize.
    let mut message: MSG = unsafe { core::mem::zeroed() };
    while !stop.load(Ordering::Acquire) {
        // SAFETY: The message pointer is writable; null HWND receives all thread messages.
        let result = unsafe { GetMessageW(ptr::addr_of_mut!(message), ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        // SAFETY: GetMessageW initialized this message.
        unsafe {
            TranslateMessage(ptr::addr_of!(message));
            DispatchMessageW(ptr::addr_of!(message));
        }
    }

    // SAFETY: Both hooks are live and uniquely owned by this observer thread.
    unsafe {
        UnhookWindowsHookEx(mouse);
        UnhookWindowsHookEx(keyboard);
    }
    clear_sender(sender);
    Ok(())
}

fn clear_sender(sender: *mut SyncSender<InputEvent>) {
    let previous = OBSERVER_SENDER.swap(ptr::null_mut(), Ordering::AcqRel);
    if previous == sender {
        // SAFETY: Hooks are absent, so no callback can read this unique allocation now.
        unsafe { drop(Box::from_raw(sender)) };
    }
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    if code == HC_ACTION && lparam != 0 {
        // SAFETY: Windows provides a KBDLLHOOKSTRUCT pointer for HC_ACTION keyboard callbacks.
        let data = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let message = u32::try_from(wparam).unwrap_or_default();
        let down = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
        let up = matches!(message, WM_KEYUP | WM_SYSKEYUP);
        if down || up {
            let extended = data.flags & LLKHF_EXTENDED != 0;
            let scan = u16::try_from(data.scanCode).unwrap_or_default();
            if let Some(hid) = scan_to_hid(scan, extended, data.vkCode) {
                send_observed(InputEvent {
                    device: InputDevice::Keyboard,
                    code: hid,
                    edge: if down { Edge::Down } else { Edge::Up },
                    source: if data.flags & LLKHF_INJECTED != 0 {
                        InputSource::Synthetic
                    } else {
                        InputSource::Physical
                    },
                });
            }
        }
    }
    // SAFETY: Passing the event onward is required for a passive low-level hook.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    if code == HC_ACTION && lparam != 0 {
        // SAFETY: Windows provides an MSLLHOOKSTRUCT pointer for HC_ACTION mouse callbacks.
        let data = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        let message = u32::try_from(wparam).unwrap_or_default();
        let translated = match message {
            WM_LBUTTONDOWN => Some((MouseButton::Left, Edge::Down)),
            WM_LBUTTONUP => Some((MouseButton::Left, Edge::Up)),
            WM_RBUTTONDOWN => Some((MouseButton::Right, Edge::Down)),
            WM_RBUTTONUP => Some((MouseButton::Right, Edge::Up)),
            WM_MBUTTONDOWN => Some((MouseButton::Middle, Edge::Down)),
            WM_MBUTTONUP => Some((MouseButton::Middle, Edge::Up)),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let button = match data.mouseData >> 16 {
                    XBUTTON1 => Some(MouseButton::Back),
                    XBUTTON2 => Some(MouseButton::Forward),
                    _ => None,
                };
                button.map(|button| {
                    (button, if message == WM_XBUTTONDOWN { Edge::Down } else { Edge::Up })
                })
            }
            _ => None,
        };
        if let Some((button, edge)) = translated {
            send_observed(InputEvent {
                device: InputDevice::MouseButton,
                code: button as u16,
                edge,
                source: if data.flags & LLMHF_INJECTED != 0 {
                    InputSource::Synthetic
                } else {
                    InputSource::Physical
                },
            });
        }
    }
    // SAFETY: Passing the event onward is required for a passive low-level hook.
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

fn send_observed(event: InputEvent) {
    let sender = OBSERVER_SENDER.load(Ordering::Acquire);
    if sender.is_null() {
        return;
    }
    // SAFETY: The observer owns the allocation until hooks are removed, and this callback runs
    // while a hook is active.
    match unsafe { &*sender }.try_send(event) {
        Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
    }
}

#[must_use]
pub const fn permission_status() -> u32 {
    PERMISSION_OBSERVE | PERMISSION_INJECT
}

#[must_use]
pub const fn request_permissions() -> u32 {
    permission_status()
}

#[allow(clippy::too_many_lines)]
const fn hid_to_scan(code: u16) -> Option<(u16, bool)> {
    Some(match code {
        0x04 => (0x1e, false),
        0x05 => (0x30, false),
        0x06 => (0x2e, false),
        0x07 => (0x20, false),
        0x08 => (0x12, false),
        0x09 => (0x21, false),
        0x0a => (0x22, false),
        0x0b => (0x23, false),
        0x0c => (0x17, false),
        0x0d => (0x24, false),
        0x0e => (0x25, false),
        0x0f => (0x26, false),
        0x10 => (0x32, false),
        0x11 => (0x31, false),
        0x12 => (0x18, false),
        0x13 => (0x19, false),
        0x14 => (0x10, false),
        0x15 => (0x13, false),
        0x16 => (0x1f, false),
        0x17 => (0x14, false),
        0x18 => (0x16, false),
        0x19 => (0x2f, false),
        0x1a => (0x11, false),
        0x1b => (0x2d, false),
        0x1c => (0x15, false),
        0x1d => (0x2c, false),
        0x1e => (0x02, false),
        0x1f => (0x03, false),
        0x20 => (0x04, false),
        0x21 => (0x05, false),
        0x22 => (0x06, false),
        0x23 => (0x07, false),
        0x24 => (0x08, false),
        0x25 => (0x09, false),
        0x26 => (0x0a, false),
        0x27 => (0x0b, false),
        0x28 => (0x1c, false),
        0x29 => (0x01, false),
        0x2a => (0x0e, false),
        0x2b => (0x0f, false),
        0x2c => (0x39, false),
        0x2d => (0x0c, false),
        0x2e => (0x0d, false),
        0x2f => (0x1a, false),
        0x30 => (0x1b, false),
        0x31 => (0x2b, false),
        0x32 | 0x64 => (0x56, false),
        0x33 => (0x27, false),
        0x34 => (0x28, false),
        0x35 => (0x29, false),
        0x36 => (0x33, false),
        0x37 => (0x34, false),
        0x38 => (0x35, false),
        0x39 => (0x3a, false),
        0x3a => (0x3b, false),
        0x3b => (0x3c, false),
        0x3c => (0x3d, false),
        0x3d => (0x3e, false),
        0x3e => (0x3f, false),
        0x3f => (0x40, false),
        0x40 => (0x41, false),
        0x41 => (0x42, false),
        0x42 => (0x43, false),
        0x43 => (0x44, false),
        0x44 => (0x57, false),
        0x45 => (0x58, false),
        0x46 => (0x37, true),
        0x47 => (0x46, false),
        0x48 => (0x45, false),
        0x49 => (0x52, true),
        0x4a => (0x47, true),
        0x4b => (0x49, true),
        0x4c => (0x53, true),
        0x4d => (0x4f, true),
        0x4e => (0x51, true),
        0x4f => (0x4d, true),
        0x50 => (0x4b, true),
        0x51 => (0x50, true),
        0x52 => (0x48, true),
        0x53 => (0x45, true),
        0x54 => (0x35, true),
        0x55 => (0x37, false),
        0x56 => (0x4a, false),
        0x57 => (0x4e, false),
        0x58 => (0x1c, true),
        0x59 => (0x4f, false),
        0x5a => (0x50, false),
        0x5b => (0x51, false),
        0x5c => (0x4b, false),
        0x5d => (0x4c, false),
        0x5e => (0x4d, false),
        0x5f => (0x47, false),
        0x60 => (0x48, false),
        0x61 => (0x49, false),
        0x62 => (0x52, false),
        0x63 => (0x53, false),
        0x65 => (0x5d, true),
        0x67 => (0x59, false),
        0x68 => (0x64, false),
        0x69 => (0x65, false),
        0x6a => (0x66, false),
        0x6b => (0x67, false),
        0x6c => (0x68, false),
        0x6d => (0x69, false),
        0x6e => (0x6a, false),
        0x6f => (0x6b, false),
        0x70 => (0x6c, false),
        0x71 => (0x6d, false),
        0x72 => (0x6e, false),
        0x73 => (0x76, false),
        0x78 => (0x68, true),
        0x7f => (0x20, true),
        0x80 => (0x30, true),
        0x81 => (0x2e, true),
        0x87 => (0x73, false),
        0x89 => (0x7d, false),
        0x90 => (0x72, false),
        0x91 => (0x71, false),
        0xe0 => (0x1d, false),
        0xe1 => (0x2a, false),
        0xe2 => (0x38, false),
        0xe3 => (0x5b, true),
        0xe4 => (0x1d, true),
        0xe5 => (0x36, false),
        0xe6 => (0x38, true),
        0xe7 => (0x5c, true),
        _ => return None,
    })
}

const fn scan_to_hid(scan: u16, extended: bool, virtual_key: u32) -> Option<u16> {
    if virtual_key == 0x13 {
        return Some(0x48);
    }
    if virtual_key == 0x2c {
        return Some(0x46);
    }
    let mut usage = 0_u16;
    while usage <= 0xe7 {
        if let Some((candidate_scan, candidate_extended)) = hid_to_scan(usage) {
            if candidate_scan == scan && candidate_extended == extended {
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
    fn supported_scan_map_round_trips() {
        for usage in 0_u16..=0xe7 {
            let Some((scan, extended)) = hid_to_scan(usage) else { continue };
            let canonical = scan_to_hid(scan, extended, 0).unwrap();
            assert_eq!(hid_to_scan(canonical), Some((scan, extended)));
        }
    }
}
