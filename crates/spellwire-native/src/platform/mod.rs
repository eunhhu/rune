use core::fmt;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    thread::JoinHandle,
};

use spellwire_core::{Injector, InputEvent};

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
    Capability::HostCallbackInjection as u32
        | Capability::NativeObservation as u32
        | Capability::NativeInjection as u32
        | Capability::HostLifecycle as u32
        | Capability::NonBlockingDelay as u32
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
pub fn start_observer(sender: SyncSender<InputEvent>) -> Result<Observer, PlatformError> {
    backend::start_observer(sender)
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
