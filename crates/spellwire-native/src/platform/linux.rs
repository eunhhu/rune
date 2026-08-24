use core::{mem::size_of, slice};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use spellwire_core::{
    Edge, Injector, InputDevice, InputEvent, InputSource, MouseButton, OutputEvent,
};

use super::{
    InputPolicy, InputSender, Observer, PlatformError, PlatformInjector, PERMISSION_INJECT,
    PERMISSION_OBSERVE,
};

const VIRTUAL_DEVICE_NAME: &str = "Spellwire Virtual Input";
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const SYN_REPORT: u16 = 0;
const REL_X: u16 = 0;
const REL_Y: u16 = 1;
const REL_HWHEEL: u16 = 6;
const REL_WHEEL: u16 = 8;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;
const BTN_SIDE: u16 = 0x113;
const BTN_EXTRA: u16 = 0x114;
const BUS_USB: u16 = 0x03;

const UINPUT_IOCTL_BASE: u64 = b'U' as u64;
const IOC_WRITE: u64 = 1;
const IOC_DIR_SHIFT: u64 = 30;
const IOC_SIZE_SHIFT: u64 = 16;
const IOC_TYPE_SHIFT: u64 = 8;
const UI_DEV_CREATE: libc::c_ulong = ioctl_none(1);
const UI_DEV_DESTROY: libc::c_ulong = ioctl_none(2);
const UI_DEV_SETUP: libc::c_ulong = ioctl_write::<libc::uinput_setup>(3);
const UI_SET_EVBIT: libc::c_ulong = ioctl_write::<libc::c_int>(100);
const UI_SET_KEYBIT: libc::c_ulong = ioctl_write::<libc::c_int>(101);
const UI_SET_RELBIT: libc::c_ulong = ioctl_write::<libc::c_int>(102);

const fn ioctl_none(number: u64) -> libc::c_ulong {
    ((UINPUT_IOCTL_BASE << IOC_TYPE_SHIFT) | number) as libc::c_ulong
}

const fn ioctl_write<T>(number: u64) -> libc::c_ulong {
    ((IOC_WRITE << IOC_DIR_SHIFT)
        | ((size_of::<T>() as u64) << IOC_SIZE_SHIFT)
        | (UINPUT_IOCTL_BASE << IOC_TYPE_SHIFT)
        | number) as libc::c_ulong
}

struct LinuxInjector {
    device: File,
}

impl LinuxInjector {
    fn create() -> Result<Self, PlatformError> {
        let device = open_uinput()?;
        let fd = device.as_raw_fd();
        set_uinput_bit(fd, UI_SET_EVBIT, EV_KEY)?;
        set_uinput_bit(fd, UI_SET_EVBIT, EV_REL)?;
        for usage in 0_u16..=0xff {
            if let Some(code) = hid_to_linux_key(usage) {
                set_uinput_bit(fd, UI_SET_KEYBIT, code)?;
            }
        }
        for code in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA] {
            set_uinput_bit(fd, UI_SET_KEYBIT, code)?;
        }
        for code in [REL_X, REL_Y, REL_HWHEEL, REL_WHEEL] {
            set_uinput_bit(fd, UI_SET_RELBIT, code)?;
        }

        // SAFETY: The all-zero value is valid for this C record; fields are initialized below.
        let mut setup: libc::uinput_setup = unsafe { core::mem::zeroed() };
        setup.id.bustype = BUS_USB;
        setup.id.vendor = 0x1209;
        setup.id.product = 0x5357;
        setup.id.version = 1;
        for (target, source) in setup.name.iter_mut().zip(VIRTUAL_DEVICE_NAME.bytes()) {
            *target = i8::try_from(source).unwrap_or_default();
        }
        // SAFETY: `setup` is a complete uinput_setup record and the fd is a writable uinput node.
        if unsafe { libc::ioctl(fd, UI_DEV_SETUP, core::ptr::addr_of!(setup)) } < 0 {
            return Err(PlatformError::Io(io::Error::last_os_error()));
        }
        // SAFETY: The uinput descriptor has all capabilities and identity configured.
        if unsafe { libc::ioctl(fd, UI_DEV_CREATE) } < 0 {
            return Err(PlatformError::Io(io::Error::last_os_error()));
        }
        wait_for_virtual_device()?;
        Ok(Self { device })
    }

    fn push_event(
        target: &mut [libc::input_event],
        len: &mut usize,
        kind: u16,
        code: u16,
        value: i32,
    ) {
        // SAFETY: An all-zero timeval is valid for uinput, which supplies its own timestamp.
        let mut event: libc::input_event = unsafe { core::mem::zeroed() };
        event.type_ = kind;
        event.code = code;
        event.value = value;
        target[*len] = event;
        *len += 1;
    }
}

fn wait_for_virtual_device() -> Result<(), PlatformError> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let registered = fs::read_dir("/sys/class/input").is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                entry.file_name().to_str().is_some_and(|name| name.starts_with("event"))
                    && fs::read_to_string(entry.path().join("device/name"))
                        .is_ok_and(|name| name.trim() == VIRTUAL_DEVICE_NAME)
            })
        });
        if registered {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(PlatformError::Initialization(
                "uinput device did not appear in /sys/class/input within one second",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

impl Drop for LinuxInjector {
    fn drop(&mut self) {
        // SAFETY: The descriptor owns a created uinput device. Destruction is idempotent at close.
        unsafe { libc::ioctl(self.device.as_raw_fd(), UI_DEV_DESTROY) };
    }
}

impl Injector for LinuxInjector {
    type Error = PlatformError;

    fn send(&mut self, events: &[OutputEvent]) -> Result<(), Self::Error> {
        // MouseWheel can expand into two relative events, plus one final synchronization record.
        const CAPACITY: usize = spellwire_core::MAX_OUTPUT_BATCH * 2 + 1;
        // SAFETY: An all-zero input_event is valid and every used slot is overwritten below.
        let mut raw: [libc::input_event; CAPACITY] = unsafe { core::mem::zeroed() };
        let mut len = 0;
        for event in events {
            match *event {
                OutputEvent::Empty => {}
                OutputEvent::Key { code, down } => {
                    let code = hid_to_linux_key(code).ok_or(PlatformError::UnsupportedKey(code))?;
                    Self::push_event(&mut raw, &mut len, EV_KEY, code, i32::from(down));
                }
                OutputEvent::MouseButton { button, down } => {
                    Self::push_event(
                        &mut raw,
                        &mut len,
                        EV_KEY,
                        mouse_button_code(button),
                        i32::from(down),
                    );
                }
                OutputEvent::MouseMove { dx, dy } => {
                    if dx != 0 {
                        Self::push_event(&mut raw, &mut len, EV_REL, REL_X, dx);
                    }
                    if dy != 0 {
                        Self::push_event(&mut raw, &mut len, EV_REL, REL_Y, dy);
                    }
                }
                OutputEvent::MouseWheel { x, y } => {
                    if x != 0 {
                        Self::push_event(&mut raw, &mut len, EV_REL, REL_HWHEEL, x);
                    }
                    if y != 0 {
                        Self::push_event(&mut raw, &mut len, EV_REL, REL_WHEEL, y);
                    }
                }
            }
        }
        if len == 0 {
            return Ok(());
        }
        Self::push_event(&mut raw, &mut len, EV_SYN, SYN_REPORT, 0);
        // SAFETY: input_event is a plain C record and `len` initialized records are contiguous.
        let bytes = unsafe {
            slice::from_raw_parts(raw.as_ptr().cast::<u8>(), len * size_of::<libc::input_event>())
        };
        self.device.write_all(bytes)?;
        Ok(())
    }
}

fn open_uinput() -> Result<File, PlatformError> {
    let mut last_error = None;
    for path in ["/dev/uinput", "/dev/input/uinput"] {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(device) => return Ok(device),
            Err(error) => last_error = Some(error),
        }
    }
    Err(PlatformError::Io(last_error.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "uinput device node does not exist")
    })))
}

fn set_uinput_bit(fd: libc::c_int, request: libc::c_ulong, value: u16) -> io::Result<()> {
    // SAFETY: These uinput ioctls accept a promoted integer capability code.
    if unsafe { libc::ioctl(fd, request, libc::c_int::from(value)) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn create_injector() -> Result<PlatformInjector, PlatformError> {
    Ok(Box::new(LinuxInjector::create()?))
}

struct ObservedDevice {
    path: PathBuf,
    file: File,
    source: InputSource,
}

pub fn start_observer(
    sender: InputSender,
    _policy: Arc<InputPolicy>,
) -> Result<Observer, PlatformError> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (setup_sender, setup_receiver) = sync_channel(1);
    let join = thread::Builder::new()
        .name("spellwire-linux-observer".into())
        .spawn(move || run_observer(sender, worker_stop, setup_sender))?;
    match setup_receiver.recv() {
        Ok(Ok(())) => Ok(Observer::new(stop, join)),
        Ok(Err(message)) => {
            let _ = join.join();
            Err(PlatformError::Initialization(message))
        }
        Err(_) => {
            let _ = join.join();
            Err(PlatformError::Initialization(
                "Linux observer exited before initialization completed",
            ))
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_observer(
    sender: InputSender,
    stop: Arc<AtomicBool>,
    setup: SyncSender<Result<(), &'static str>>,
) -> Result<(), PlatformError> {
    let mut devices = discover_devices()?;
    if devices.is_empty() {
        let message = "no readable /dev/input/event* devices; install the documented udev rule";
        let _ = setup.send(Err(message));
        return Err(PlatformError::PermissionDenied("Linux evdev read access"));
    }
    let _ = setup.send(Ok(()));
    let mut last_scan = Instant::now();
    let mut poll_fds = build_poll_fds(&devices);
    while !stop.load(Ordering::Acquire) {
        for poll_fd in &mut poll_fds {
            poll_fd.revents = 0;
        }
        // SAFETY: poll_fds is a live writable array for this bounded call.
        let poll_status =
            unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 25) };
        if poll_status < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(PlatformError::Io(error));
            }
        } else if poll_status > 0 {
            let mut removed = false;
            for index in (0..devices.len()).rev() {
                let revents = poll_fds[index].revents;
                if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                    devices.remove(index);
                    removed = true;
                } else if revents & libc::POLLIN != 0 {
                    read_device_events(&mut devices[index], &sender)?;
                }
            }
            if removed {
                poll_fds = build_poll_fds(&devices);
            }
        }
        if last_scan.elapsed() >= Duration::from_secs(1) {
            if refresh_devices(&mut devices)? {
                poll_fds = build_poll_fds(&devices);
            }
            last_scan = Instant::now();
        }
    }
    Ok(())
}

fn read_device_events(
    device: &mut ObservedDevice,
    sender: &InputSender,
) -> Result<(), PlatformError> {
    // SAFETY: An all-zero input_event array is valid writable storage for read().
    let mut events: [libc::input_event; 32] = unsafe { core::mem::zeroed() };
    // SAFETY: The byte slice spans the exact writable event array storage.
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            events.as_mut_ptr().cast::<u8>(),
            events.len() * size_of::<libc::input_event>(),
        )
    };
    match device.file.read(bytes) {
        Ok(0) => Ok(()),
        Ok(count) => {
            for event in &events[..count / size_of::<libc::input_event>()] {
                if event.type_ != EV_KEY {
                    continue;
                }
                let Some(edge) = linux_key_edge(event.value) else { continue };
                let translated = linux_key_to_hid(event.code)
                    .map(|code| (InputDevice::Keyboard, code))
                    .or_else(|| {
                        linux_button(event.code)
                            .map(|button| (InputDevice::MouseButton, button as u16))
                    });
                let Some((input_device, code)) = translated else { continue };
                let input = InputEvent { device: input_device, code, edge, source: device.source };
                let _ = sender.try_send(input);
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(PlatformError::Io(error)),
    }
}

const fn linux_key_edge(value: i32) -> Option<Edge> {
    match value {
        0 => Some(Edge::Up),
        1 | 2 => Some(Edge::Down),
        _ => None,
    }
}

fn discover_devices() -> Result<Vec<ObservedDevice>, PlatformError> {
    let mut devices = Vec::new();
    let entries = match fs::read_dir("/dev/input") {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(devices),
        Err(error) => return Err(PlatformError::Io(error)),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("event"))
        {
            if let Ok(device) = open_event_device(path) {
                devices.push(device);
            }
        }
    }
    Ok(devices)
}

fn build_poll_fds(devices: &[ObservedDevice]) -> Vec<libc::pollfd> {
    devices
        .iter()
        .map(|device| libc::pollfd {
            fd: device.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect()
}

fn refresh_devices(devices: &mut Vec<ObservedDevice>) -> Result<bool, PlatformError> {
    let previous_len = devices.len();
    devices.retain(|device| device.path.exists());
    let mut changed = devices.len() != previous_len;
    let known: HashSet<PathBuf> = devices.iter().map(|device| device.path.clone()).collect();
    for device in discover_devices()? {
        if !known.contains(&device.path) {
            devices.push(device);
            changed = true;
        }
    }
    Ok(changed)
}

fn open_event_device(path: PathBuf) -> io::Result<ObservedDevice> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&path)?;
    let source = if event_device_name(&path).as_deref() == Some(VIRTUAL_DEVICE_NAME) {
        InputSource::Synthetic
    } else {
        InputSource::Physical
    };
    Ok(ObservedDevice { path, file, source })
}

fn event_device_name(path: &Path) -> Option<String> {
    let event = path.file_name()?.to_str()?;
    fs::read_to_string(format!("/sys/class/input/{event}/device/name"))
        .ok()
        .map(|name| name.trim().to_owned())
}

#[must_use]
pub fn permission_status() -> u32 {
    let mut status = 0;
    if discover_devices().is_ok_and(|devices| !devices.is_empty()) {
        status |= PERMISSION_OBSERVE;
    }
    if open_uinput().is_ok() {
        status |= PERMISSION_INJECT;
    }
    status
}

#[must_use]
pub fn request_permissions() -> u32 {
    permission_status()
}

const fn mouse_button_code(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => BTN_LEFT,
        MouseButton::Right => BTN_RIGHT,
        MouseButton::Middle => BTN_MIDDLE,
        MouseButton::Back => BTN_SIDE,
        MouseButton::Forward => BTN_EXTRA,
    }
}

const fn linux_button(code: u16) -> Option<MouseButton> {
    match code {
        BTN_LEFT => Some(MouseButton::Left),
        BTN_RIGHT => Some(MouseButton::Right),
        BTN_MIDDLE => Some(MouseButton::Middle),
        BTN_SIDE => Some(MouseButton::Back),
        BTN_EXTRA => Some(MouseButton::Forward),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
const fn hid_to_linux_key(code: u16) -> Option<u16> {
    Some(match code {
        0x04 => 30,
        0x05 => 48,
        0x06 => 46,
        0x07 => 32,
        0x08 => 18,
        0x09 => 33,
        0x0a => 34,
        0x0b => 35,
        0x0c => 23,
        0x0d => 36,
        0x0e => 37,
        0x0f => 38,
        0x10 => 50,
        0x11 => 49,
        0x12 => 24,
        0x13 => 25,
        0x14 => 16,
        0x15 => 19,
        0x16 => 31,
        0x17 => 20,
        0x18 => 22,
        0x19 => 47,
        0x1a => 17,
        0x1b => 45,
        0x1c => 21,
        0x1d => 44,
        0x1e => 2,
        0x1f => 3,
        0x20 => 4,
        0x21 => 5,
        0x22 => 6,
        0x23 => 7,
        0x24 => 8,
        0x25 => 9,
        0x26 => 10,
        0x27 => 11,
        0x28 => 28,
        0x29 => 1,
        0x2a => 14,
        0x2b => 15,
        0x2c => 57,
        0x2d => 12,
        0x2e => 13,
        0x2f => 26,
        0x30 => 27,
        0x31 => 43,
        0x32 | 0x64 => 86,
        0x33 => 39,
        0x34 => 40,
        0x35 => 41,
        0x36 => 51,
        0x37 => 52,
        0x38 => 53,
        0x39 => 58,
        0x3a => 59,
        0x3b => 60,
        0x3c => 61,
        0x3d => 62,
        0x3e => 63,
        0x3f => 64,
        0x40 => 65,
        0x41 => 66,
        0x42 => 67,
        0x43 => 68,
        0x44 => 87,
        0x45 => 88,
        0x46 => 99,
        0x47 => 70,
        0x48 => 119,
        0x49 => 110,
        0x4a => 102,
        0x4b => 104,
        0x4c => 111,
        0x4d => 107,
        0x4e => 109,
        0x4f => 106,
        0x50 => 105,
        0x51 => 108,
        0x52 => 103,
        0x53 => 69,
        0x54 => 98,
        0x55 => 55,
        0x56 => 74,
        0x57 => 78,
        0x58 => 96,
        0x59 => 79,
        0x5a => 80,
        0x5b => 81,
        0x5c => 75,
        0x5d => 76,
        0x5e => 77,
        0x5f => 71,
        0x60 => 72,
        0x61 => 73,
        0x62 => 82,
        0x63 => 83,
        0x65 => 127,
        0x67 => 117,
        0x68 => 183,
        0x69 => 184,
        0x6a => 185,
        0x6b => 186,
        0x6c => 187,
        0x6d => 188,
        0x6e => 189,
        0x6f => 190,
        0x70 => 191,
        0x71 => 192,
        0x72 => 193,
        0x73 => 194,
        0x78 => 128,
        0x7f => 113,
        0x80 => 115,
        0x81 => 114,
        0x87 => 89,
        0x89 => 124,
        0x90 => 122,
        0x91 => 123,
        0xe0 => 29,
        0xe1 => 42,
        0xe2 => 56,
        0xe3 => 125,
        0xe4 => 97,
        0xe5 => 54,
        0xe6 => 100,
        0xe7 => 126,
        _ => return None,
    })
}

const fn linux_key_to_hid(key: u16) -> Option<u16> {
    let mut usage = 0_u16;
    while usage <= 0xff {
        if let Some(candidate) = hid_to_linux_key(usage) {
            if candidate == key {
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
    fn supported_linux_key_map_round_trips() {
        for usage in 0_u16..=0xff {
            let Some(key) = hid_to_linux_key(usage) else { continue };
            let canonical = linux_key_to_hid(key).unwrap();
            assert_eq!(hid_to_linux_key(canonical), Some(key));
        }
    }

    #[test]
    fn preserves_linux_repeat_as_a_second_down_transition() {
        assert_eq!(linux_key_edge(0), Some(Edge::Up));
        assert_eq!(linux_key_edge(1), Some(Edge::Down));
        assert_eq!(linux_key_edge(2), Some(Edge::Down));
        assert_eq!(linux_key_edge(-1), None);
    }
}
