#!/usr/bin/env bun

import { mkdir } from "node:fs/promises";
import { basename, dirname, extname, join, resolve } from "node:path";
import { compileSource } from "./compiler/compiler";
import { encodeModule } from "./compiler/encode";
import { NativeHost, NativePermission } from "./native";

const VERSION = "0.1.0";
const DEFAULT_INPUT = "src/main.spellwire.ts";
const HELP = `Spellwire ${VERSION}

Usage:
  spellwire run [input.spellwire.ts|input.spellwire.bin]
  spellwire watch [input.spellwire.ts|input.spellwire.bin]
  spellwire compile [input.spellwire.ts] [output.spellwire.bin]
  spellwire --help
  spellwire --version

Defaults:
  input   src/main.spellwire.ts
  output  next to input as <name>.spellwire.bin

Advanced options:
  --library <path>   use an explicit native library
  --manifest <path>  use an explicit manifest for compiled input

run/watch prepare global-input permissions automatically before native startup.
`;

export async function runCli(args: string[]): Promise<void> {
  const command = args[0];

  if (command === "--help" || command === "-h" || command === "help") {
    console.log(HELP);
    return;
  }

  if (command === "--version" || command === "-v") {
    console.log(VERSION);
    return;
  }

  if (command === "run") {
    await runNative(args.slice(1), false);
    return;
  }

  if (command === "watch") {
    await runNative(args.slice(1), true);
    return;
  }

  if (command === "compile") {
    await compileProgram(args.slice(1));
    return;
  }

  if (command === undefined) {
    console.log(HELP);
    return;
  }

  throw new Error(`unknown Spellwire command: ${command}\nRun spellwire --help for usage.`);
}

async function compileProgram(args: string[]): Promise<void> {
  assertKnownOptions(args, new Set());
  const positional = positionalArguments(args, new Set());
  if (positional.length > 2) {
    throw new Error("spellwire compile accepts one input and one optional output");
  }

  const input = positional[0] ?? DEFAULT_INPUT;

  const absolute = resolve(input);
  if (!(await Bun.file(absolute).exists())) {
    throw new Error(`input file does not exist: ${absolute}`);
  }

  const extension = extname(absolute);
  const stem = basename(absolute, extension).replace(/\.spellwire$/, "");
  const output = resolve(positional[1] ?? join(dirname(absolute), `${stem}.spellwire.bin`));
  const result = compileSource(await Bun.file(absolute).text(), { fileName: absolute });

  await mkdir(dirname(output), { recursive: true });
  await Bun.write(output, encodeModule(result.module));

  const states = Object.fromEntries(
    result.module.states.map((state) => [
      state.name,
      { slot: state.slot, kind: state.kind },
    ]),
  );

  await Bun.write(
    `${output}.json`,
    `${JSON.stringify(
      {
        version: 1,
        input: absolute,
        binary: output,
        handlers: result.module.handlers.length,
        instructions: result.module.code.length,
        states,
      },
      null,
      2,
    )}\n`,
  );

  console.log(
    `compiled ${input}: ${result.module.handlers.length} handlers, ` +
      `${result.module.states.length} persistent states, ` +
      `${result.module.code.length} instructions`,
  );
  console.log(output);
}

async function runNative(args: string[], watchMode: boolean): Promise<void> {
  const valuedOptions = new Set(["--library", "--manifest"]);
  assertKnownOptions(args, new Set(["--library", "--manifest"]));
  const positional = positionalArguments(args, valuedOptions);
  if (positional.length > 1) {
    throw new Error("spellwire run/watch accepts one input program");
  }
  const input = positional[0] ?? DEFAULT_INPUT;
  const library = optionValue(args, "--library");
  const manifest = optionValue(args, "--manifest");
  const host = await NativeHost.load(input, {
    ...(library ? { nativeLibraryPath: library } : {}),
    ...(manifest ? { manifestPath: manifest } : {}),
  });
  let watcher: ReturnType<NativeHost["watch"]> | undefined;
  try {
    preparePermissions(host);
    host.start();
    if (watchMode) {
      watcher = host.watch({
        onReload: () => console.log("reloaded"),
        onError: (error) => console.error(`reload failed: ${error.message}`),
      });
    }
    console.log(
      watchMode
        ? `watching ${resolve(input)} (hot reload enabled; press Ctrl+C to stop)`
        : `running ${resolve(input)} (press Ctrl+C to stop)`,
    );
    await waitForSignal();
  } finally {
    watcher?.close();
    host.close();
  }
}

function preparePermissions(host: NativeHost): void {
  const required = NativePermission.Observe | NativePermission.Inject;
  let permissions = host.permissionStatus();
  if ((permissions & required) === required) return;

  console.log("Spellwire needs global input permissions; requesting them now...");
  permissions = host.requestPermissions();
  if ((permissions & required) === required) return;

  const missing = [
    (permissions & NativePermission.Observe) === 0 ? "observation" : undefined,
    (permissions & NativePermission.Inject) === 0 ? "injection" : undefined,
  ].filter((value): value is string => value !== undefined);
  throw new Error(permissionHelp(missing));
}

function permissionHelp(missing: readonly string[]): string {
  const prefix = `missing global input permission: ${missing.join(" and ")}.`;
  if (process.platform === "darwin") {
    return (
      `${prefix} Grant Input Monitoring and Accessibility to the app running Bun in ` +
      "System Settings > Privacy & Security, then run the command again."
    );
  }
  if (process.platform === "linux") {
    return (
      `${prefix} Grant read access to /dev/input/event* and write access to /dev/uinput; ` +
      "see the Linux section of the Spellwire Platform Verification Guide."
    );
  }
  return `${prefix} Check the current desktop session and process integrity level.`;
}

function assertKnownOptions(args: readonly string[], known: ReadonlySet<string>): void {
  for (const argument of args) {
    if (argument.startsWith("--") && !known.has(argument)) {
      throw new Error(`unknown Spellwire option: ${argument}`);
    }
  }
}

function positionalArguments(args: string[], valuedOptions: ReadonlySet<string>): string[] {
  const positional: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === undefined) continue;
    if (valuedOptions.has(argument)) {
      const value = args[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
      index += 1;
    } else if (!argument.startsWith("--")) {
      positional.push(argument);
    }
  }
  return positional;
}

function optionValue(args: string[], option: string): string | undefined {
  const index = args.indexOf(option);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) throw new Error(`${option} requires a value`);
  return value;
}

function waitForSignal(): Promise<void> {
  return new Promise((resolveSignal) => {
    const finish = (): void => {
      process.off("SIGINT", finish);
      process.off("SIGTERM", finish);
      resolveSignal();
    };
    process.once("SIGINT", finish);
    process.once("SIGTERM", finish);
  });
}

if (import.meta.main) {
  try {
    await runCli(Bun.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
