#!/usr/bin/env bun

import {
  EventSource,
  DynamicInputLane,
  InputDevice,
  InputEdge,
  Key,
  NativeHost,
  NativePermission,
} from "../packages/spellwire/src/index";

const host = await NativeHost.load("examples/platform-loopback.spellwire.ts");
try {
  const permissions = Bun.argv.includes("--request-permissions")
    ? host.requestPermissions()
    : host.permissionStatus();
  const required = NativePermission.Observe | NativePermission.Inject;
  if ((permissions & required) !== required) {
    throw new Error(
      "native observe/inject permissions are missing; rerun with --request-permissions after platform setup",
    );
  }
  const lane = new DynamicInputLane(64);
  host.attachDynamicLane(lane);
  host.start();
  const start = Bun.nanoseconds();
  host.dispatch(InputDevice.Keyboard, Key.F19, InputEdge.Down, EventSource.Physical);
  const deadline = performance.now() + 2_000;
  while (host.state("observed").get() !== 1 && performance.now() < deadline) {
    await Bun.sleep(5);
  }
  const observed = host.state("observed").get();
  if (observed !== 1) throw new Error(`synthetic F20 loopback timed out (state=${observed})`);
  await Bun.sleep(25);
  lane.drain();

  let releasedUps = 0;
  lane.on(InputDevice.Keyboard, Key.F20, InputEdge.Up, () => {
    releasedUps += 1;
  });
  host.dispatch(InputDevice.Keyboard, Key.F18, InputEdge.Down, EventSource.Physical);
  await Bun.sleep(25);
  lane.drain();
  if (releasedUps !== 0) throw new Error("delayed F20 released before its deadline");
  await host.reload();
  const releaseDeadline = performance.now() + 1_000;
  while (releasedUps === 0 && performance.now() < releaseDeadline) {
    await Bun.sleep(5);
    lane.drain();
  }
  if (releasedUps !== 1) {
    throw new Error(`reload did not release the held synthetic F20 (ups=${releasedUps})`);
  }
  console.log(
    JSON.stringify({
      platform: process.platform,
      arch: process.arch,
      loopback: "ok",
      observed,
      reloadReleasedHeldInput: true,
      elapsedUs: Math.round((Bun.nanoseconds() - start) / 1_000),
    }),
  );
} finally {
  host.close();
}
