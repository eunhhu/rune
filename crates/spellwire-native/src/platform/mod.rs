#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[allow(dead_code)] // Reserved ABI capability bits are advertised as backends land.
pub enum Capability {
    None = 0,
    HostCallbackInjection = 1 << 0,
    NativeObservation = 1 << 1,
    NativeInjection = 1 << 2,
    NativeOverlay = 1 << 3,
}

#[must_use]
pub const fn current_capabilities() -> u32 {
    // The runtime and host callback ABI are implemented. Direct OS observation,
    // injection and overlay backends are intentionally not advertised until their
    // latency and permission behavior are tested independently on each platform.
    Capability::HostCallbackInjection as u32
}
