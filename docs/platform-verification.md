# Platform Verification Guide

[한국어](platform-verification.ko.md)

Use this guide on every release target. It distinguishes source checks, native OS loopback, submission timing, overlay startup, and physical end-to-end latency. Passing one layer does not prove the next.

Generated projects do not require a separate permission workflow: `bun run start` and `bun run watch` prepare permissions automatically. This release checklist uses the hidden-compatible `permissions` diagnostic command only because verification needs the raw ABI, capability, and permission values.

## What each check proves

| Check | What it proves | What it does not prove |
| --- | --- | --- |
| `bun run check` | TypeScript/Rust compilation, tests, formatting, Clippy | Device permissions or live OS behavior |
| `permissions` | Native library loads; current process can query/open required resources | Windows injection into every integrity level |
| `test:platform-loopback` | VM output reaches OS injection, returns through global observation, keeps synthetic classification, and updates VM state | Physical keyboard latency or target-application receipt |
| `bench:platform` | Time until the native OS submission call returns | Device delivery, compositor, application polling |
| overlay `--smoke` | Window, GPU surface, transparency mode, and event loop initialize | Appearance on every compositor or multi-monitor layout |

## Common preflight

Run from the repository root and save the output with your report:

```bash
git rev-parse HEAD
bun --version
rustc --version
cargo --version
```

Expected minimums are Bun 1.4.0 and Rust 1.81. Then prepare the checkout:

```bash
bun install --frozen-lockfile
bun run check
bun run build:native
```

If `bun run check` fails, stop and fix that portable failure before interpreting platform results.

The loopback test injects global synthetic F20 events. It does not type text or click a mouse button, but another application may have an F20 shortcut. Close or disable any F20-sensitive application before running it. The platform benchmark submits zero-delta mouse movement batches and should not move the pointer.

## macOS verification

### 1. Grant the two privacy permissions

Run:

```bash
bun packages/spellwire/src/cli.ts permissions --request
```

macOS may open or update two entries:

- **System Settings → Privacy & Security → Input Monitoring** for observation;
- **System Settings → Privacy & Security → Accessibility** for injection.

Grant the permissions to the application that actually launches Bun. That may be Terminal, iTerm, an IDE, or Codex. If status remains stale, fully quit and reopen that application; restarting only the shell command may not refresh macOS privacy state.

Recheck without prompting:

```bash
bun packages/spellwire/src/cli.ts permissions
```

Expected shape:

```text
library: /.../target/release/libspellwire_native.dylib
ABI: 3
capabilities: 0x37
observe: granted
inject: granted
```

### 2. Run native loopback

```bash
bun run test:platform-loopback
```

Expected JSON shape:

```json
{"platform":"darwin","arch":"arm64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

Intel Macs report `x64`. The exact `elapsedUs` value varies and is not a latency benchmark: this scenario intentionally sleeps and polls while checking reload cleanup.

### 3. Run the submission benchmark

```bash
bun run bench:platform -- 10000
```

The command reports p50, p95, p99, p999, and maximum nanoseconds for zero-delta mouse batches. It measures the return of `CGEventPost` submission work only. Do not present it as physical switch-to-application latency.

### 4. Start the overlay smoke test

```bash
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

Success prints one JSON line and exits after roughly 350 ms:

```json
{"event":"ready","width":3420,"height":2214,"alphaMode":"PostMultiplied"}
```

Dimensions and alpha mode depend on the monitor and GPU. Success requires positive dimensions and a valid `alphaMode`; the example values are not required.

## Windows verification

Use a normal PowerShell window first. Build and query the runtime:

```powershell
bun install --frozen-lockfile
bun run check
bun run build:native
bun packages/spellwire/src/cli.ts permissions
```

Expected library suffix is `target\release\spellwire_native.dll`. Windows currently reports both permission bits as granted because low-level hooks and `SendInput` have no preflight prompt.

That status does not bypass User Interface Privilege Isolation. A normal Spellwire process cannot inject into an administrator-elevated target. Verify first against a normal desktop application. If elevated-target testing is required, run Spellwire at the same integrity level and record that fact; never treat elevation as a general installation requirement.

Run the native checks:

```powershell
bun run test:platform-loopback
bun run bench:platform -- 10000
.\target\release\spellwire-overlay.exe --smoke
bun run test:overlay-live
```

Expected loopback fields:

```json
{"platform":"win32","arch":"x64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

On Windows arm64, `arch` should be `arm64`. If loopback works in a normal app but not an elevated app, that is expected UIPI behavior rather than a mapping failure.

## Linux verification

The Linux backend reads evdev and creates one uinput device. These interfaces expose global input and therefore require deliberately granted device access.

### 1. Inspect device availability

```bash
ls -l /dev/input/event* /dev/uinput
bun packages/spellwire/src/cli.ts permissions
```

`observe: granted` means at least one readable evdev device was discovered. `inject: granted` means `/dev/uinput` opened successfully. `permissions --request` does not install rules or display a prompt on Linux; it only rechecks the same resources.

### 2. Review and optionally install the supplied udev rule

Read the rule before installing it:

```bash
cat packaging/linux/99-spellwire-input.rules
```

It grants the active local seat through `uaccess`; it does not make devices world-readable. Installing into `/etc` changes system configuration and requires administrator approval:

```bash
sudo install -m 0644 packaging/linux/99-spellwire-input.rules /etc/udev/rules.d/99-spellwire-input.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=input
```

Log out and back in, or reconnect the relevant device, if access does not update. Headless sessions and distributions without systemd-logind `uaccess` handling may require a distribution-specific group or service rule; do not use `chmod 666` as a persistent workaround.

Recheck:

```bash
bun packages/spellwire/src/cli.ts permissions
```

### 3. Run loopback and benchmark

```bash
bun run test:platform-loopback
bun run bench:platform -- 10000
```

Expected loopback fields:

```json
{"platform":"linux","arch":"x64","loopback":"ok","observed":1,"reloadReleasedHeldInput":true,"elapsedUs":123456}
```

The backend waits for the uinput device to register, identifies Spellwire's virtual device by name, and classifies its returning events as synthetic. A timeout can mean that udev has not exposed the new event node to the same session yet.

### 4. Verify the overlay in the intended graphical session

```bash
printf 'XDG_SESSION_TYPE=%s DISPLAY=%s WAYLAND_DISPLAY=%s\n' \
  "${XDG_SESSION_TYPE:-}" "${DISPLAY:-}" "${WAYLAND_DISPLAY:-}"
target/release/spellwire-overlay --smoke
bun run test:overlay-live
```

Repeat on each supported desktop/compositor. X11 and Wayland environments differ; winit cannot provide one universal Wayland layer-shell guarantee. Record desktop name, version, session type, monitor arrangement, and whether the window is transparent, topmost, and click-through.

## Understand the loopback scenario

`scripts/platform-loopback.ts` performs these checks in order:

1. load `examples/platform-loopback.spellwire.ts` through ABI v4;
2. require both observe and inject permissions;
3. attach a 64-record `DynamicInputLane`;
4. explicitly dispatch physical-source F19 into the VM;
5. have the VM inject tagged F20 through the real OS backend;
6. observe the returning synthetic F20 and update named state to `1`;
7. create a delayed held F20 sequence from F18;
8. reload the program before the delayed release deadline;
9. verify reload emitted exactly one release for the held synthetic F20;
10. close the host in `finally`.

The first F19 is an explicit test dispatch, not a physical keystroke. The F20 injection and returning observation do traverse the platform backend. This design makes the test repeatable without requiring a person to press a key at an exact time.

## Interpret benchmark output

Example:

```text
Spellwire platform submission benchmark (10000 zero-delta mouse batches)
p50    150000 ns
p95    250000 ns
p99    600000 ns
p999   900000 ns
max   1200000 ns

Scope: native OS submission call return; device delivery and application polling excluded.
```

Use the percentiles to compare regressions on the same OS, hardware, power state, and background load. Do not compare a warm desktop run directly with a VM, remote desktop session, battery-saver run, or another backend and call the difference end-to-end latency.

Physical latency needs a separate measurement path, such as an externally timestamped switch actuator or target-application instrumentation. Include USB polling, OS delivery, scheduler delay, compositor behavior where relevant, and application polling in that report.

## Failure matrix

| Failure | Likely cause | Next check |
| --- | --- | --- |
| Native library not found | Native build missing or wrong architecture | Confirm file name under `target/release`; remove stale path overrides |
| ABI mismatch | JS and native artifacts came from different commits | Rebuild from one checkout and rerun `permissions` |
| macOS `observe: missing` | Input Monitoring not granted to launcher | Check the correct app entry, quit it fully, reopen |
| macOS `inject: missing` | Accessibility not granted to launcher | Check Accessibility, then restart launcher |
| Windows loopback timeout | Hook setup failed, session boundary, or integrity mismatch | Test in a normal local desktop session at matching integrity |
| Linux `observe: missing` | No readable `/dev/input/event*` device | Inspect ownership/ACL and active-seat udev application |
| Linux `inject: missing` | `/dev/uinput` absent or not writable | Load/enable uinput as appropriate for the distribution; inspect ACL |
| Linux loopback timeout after injection | Virtual device registration/ACL delay | Inspect `/sys/class/input/*/device/name` and udev events |
| Overlay exits before ready | No GPU adapter/surface or no graphical session | Check stderr, session variables, drivers, remote/headless status |
| Overlay is not topmost on Wayland | Compositor policy lacks required semantics | Record compositor; evaluate compositor-specific layer-shell integration |
| Unsupported HID usage | Platform map intentionally rejects the key | Use a supported `Key`; report keyboard layout and desired usage |

## Copyable verification report

Paste this template into an issue or test handoff. Preserve raw output where possible:

```text
Spellwire commit:
OS edition/version:
CPU architecture:
Physical or virtual machine:
Bun version:
Rust version:

Permission output:
  library:
  ABI:
  capabilities:
  observe:
  inject:

Loopback JSON:

Platform benchmark (sample count and full percentiles):

Overlay ready JSON:
Overlay live-state JSON:
Overlay visually transparent/topmost/click-through:
Monitor count and scaling:

Linux only:
  distribution/kernel:
  desktop/compositor:
  XDG_SESSION_TYPE:

Windows only:
  Spellwire integrity level:
  target integrity level:

Unexpected stderr/errors:
Notes:
```

Current checked evidence and remaining target gates are tracked in [Runtime Verification](runtime-verification.md) and [Implementation Status](status.md).
