import {
  NativeHost,
  NativePermission,
  type NativeHostOptions,
  type NativeHostWatcher,
  type NativeStateSnapshot,
} from "./native";
import { Overlay, ui, type OverlayChild, type OverlayMountOptions } from "./overlay-ui";

export interface SpellwireStartOptions extends NativeHostOptions {
  readonly input?: string;
  readonly watch?: boolean;
  readonly debounceMs?: number;
  readonly preserveState?: boolean;
  readonly requestPermissions?: boolean;
  readonly onReload?: () => void;
  readonly onError?: (error: Error) => void;
  readonly overlay?: (state: NativeStateSnapshot) => OverlayChild;
  readonly overlayOptions?: Omit<OverlayMountOptions, "renderer">;
}

/** One lifecycle owner for native input, hot reload, state-driven UI, and safe shutdown. */
export class Spellwire {
  readonly host: NativeHost;
  readonly overlay: Overlay | undefined;

  readonly #watcher: NativeHostWatcher | undefined;
  #closed = false;

  private constructor(
    host: NativeHost,
    overlay: Overlay | undefined,
    watcher: NativeHostWatcher | undefined,
  ) {
    this.host = host;
    this.overlay = overlay;
    this.#watcher = watcher;
  }

  static async start(options: SpellwireStartOptions = {}): Promise<Spellwire> {
    const host = await NativeHost.load(options.input ?? "src/main.spellwire.ts", {
      ...(options.nativeLibraryPath === undefined
        ? {}
        : { nativeLibraryPath: options.nativeLibraryPath }),
      ...(options.manifestPath === undefined ? {} : { manifestPath: options.manifestPath }),
    });
    let overlay: Overlay | undefined;
    let watcher: NativeHostWatcher | undefined;
    try {
      if (options.requestPermissions ?? true) preparePermissions(host);
      host.start();
      if (options.overlay) {
        overlay = await Overlay.mount(ui.bind(host, options.overlay), {
          ...options.overlayOptions,
          ...(options.overlayOptions?.onError === undefined && options.onError !== undefined
            ? { onError: options.onError }
            : {}),
        });
      }
      if (options.watch) {
        watcher = host.watch({
          ...(options.debounceMs === undefined ? {} : { debounceMs: options.debounceMs }),
          ...(options.preserveState === undefined
            ? {}
            : { preserveState: options.preserveState }),
          ...(options.onReload === undefined ? {} : { onReload: options.onReload }),
          ...(options.onError === undefined ? {} : { onError: options.onError }),
        });
      }
      return new Spellwire(host, overlay, watcher);
    } catch (error) {
      watcher?.close();
      await overlay?.close().catch(() => undefined);
      host.close();
      throw error;
    }
  }

  refreshOverlay(): Promise<number> {
    this.#assertOpen();
    return this.overlay?.refresh() ?? Promise.resolve(0);
  }

  async untilSignal(): Promise<void> {
    this.#assertOpen();
    await waitForSignal();
    await this.close();
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#watcher?.close();
    try {
      await this.overlay?.close();
    } finally {
      this.host.close();
    }
  }

  #assertOpen(): void {
    if (this.#closed) throw new Error("Spellwire app is closed");
  }
}

function preparePermissions(host: NativeHost): void {
  const required = NativePermission.Observe | NativePermission.Inject;
  let permissions = host.permissionStatus();
  if ((permissions & required) === required) return;
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
