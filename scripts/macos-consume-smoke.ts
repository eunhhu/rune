#!/usr/bin/env bun

import { NativeHost, NativePermission } from "../packages/spellwire/src/index";

if (process.platform !== "darwin") {
  throw new Error("macOS consume smoke requires CoreGraphics");
}

async function runProbe(): Promise<number> {
  const child = Bun.spawn(["xcrun", "swift", "scripts/macos-consume-probe.swift"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const timeout = setTimeout(() => child.kill(), 10_000);
  const [exitCode, output, errorOutput] = await Promise.all([
    child.exited.finally(() => clearTimeout(timeout)),
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  if (exitCode !== 0) {
    throw new Error(`CoreGraphics consume probe failed: ${errorOutput.trim()}`);
  }
  const probe = JSON.parse(output) as { forwardedTransitions?: unknown };
  if (!Number.isSafeInteger(probe.forwardedTransitions)) {
    throw new Error(`CoreGraphics consume probe returned malformed output: ${output.trim()}`);
  }
  return probe.forwardedTransitions as number;
}

const host = await NativeHost.load("examples/consume-smoke.spellwire.ts");
try {
  const permissions = host.permissionStatus();
  const required = NativePermission.Observe | NativePermission.Inject;
  if ((permissions & required) !== required) {
    throw new Error("macOS Input Monitoring and Accessibility permissions are required");
  }
  const baselineTransitions = await runProbe();
  if (baselineTransitions !== 2) {
    throw new Error(`consume probe baseline failed: expected 2, got ${baselineTransitions}`);
  }
  host.start();
  const inactiveTransitions = await runProbe();
  if (inactiveTransitions !== 2 || host.state("hits").get() !== 0) {
    throw new Error(
      `inactive gate failed: ${JSON.stringify({ inactiveTransitions, hits: host.state("hits").get() })}`,
    );
  }
  host.state("enabled").set(true);
  const forwardedTransitions = await runProbe();
  const deadline = performance.now() + 1_000;
  while (host.state("hits").get() !== 1 && performance.now() < deadline) {
    await Bun.sleep(5);
  }
  const hits = host.state("hits").get();
  if (hits !== 1 || forwardedTransitions !== 0) {
    throw new Error(
      `consume failed: ${JSON.stringify({ hits, forwardedTransitions })}`,
    );
  }
  console.log(JSON.stringify({
    platform: process.platform,
    arch: process.arch,
    baselineTransitions,
    inactiveTransitions,
    nativeHandlerHits: hits,
    forwardedTransitions,
    originalInput: "suppressed",
  }));
} finally {
  host.close();
}
