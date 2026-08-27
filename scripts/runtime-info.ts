#!/usr/bin/env bun

import {
  inspectNativeRuntime,
  NativeCapability,
  NativePermission,
} from "../packages/spellwire/src/index";

const requestPermissions = Bun.argv.includes("--request-permissions");
const runtime = inspectNativeRuntime({ requestPermissions });

const enabledCapabilities = Object.entries(NativeCapability)
  .filter(([, flag]) => (runtime.capabilities & flag) !== 0)
  .map(([name]) => name);

console.log(
  JSON.stringify(
    {
      abiVersion: runtime.abiVersion,
      nativeLibraryPath: runtime.nativeLibraryPath,
      capabilities: {
        mask: `0x${runtime.capabilities.toString(16)}`,
        enabled: enabledCapabilities,
      },
      permissions: {
        mask: `0x${runtime.permissions.toString(16)}`,
        observe: (runtime.permissions & NativePermission.Observe) !== 0,
        inject: (runtime.permissions & NativePermission.Inject) !== 0,
      },
    },
    null,
    2,
  ),
);
