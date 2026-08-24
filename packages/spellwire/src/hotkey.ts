import { Key, Modifier, MouseButton } from "./keys";

export interface ParsedHotkey {
  readonly device: "keyboard" | "mouse";
  readonly code: number;
  readonly modifiers: number;
}

const keyNames = new Map<string, number>();
for (const [name, value] of Object.entries(Key)) {
  if (typeof value === "number") keyNames.set(normalize(name), value);
}

const keyAliases = new Map<string, number>([
  ["esc", Key.Escape],
  ["return", Key.Enter],
  ["del", Key.Delete],
  ["ins", Key.Insert],
  ["pgup", Key.PageUp],
  ["pgdn", Key.PageDown],
  ["spacebar", Key.Space],
  ["up", Key.ArrowUp],
  ["down", Key.ArrowDown],
  ["left", Key.ArrowLeft],
  ["right", Key.ArrowRight],
]);

const mouseAliases = new Map<string, MouseButton>([
  ["lbutton", MouseButton.Left],
  ["mouseleft", MouseButton.Left],
  ["rbutton", MouseButton.Right],
  ["mouseright", MouseButton.Right],
  ["mbutton", MouseButton.Middle],
  ["mousemiddle", MouseButton.Middle],
  ["xbutton1", MouseButton.Back],
  ["mouseback", MouseButton.Back],
  ["xbutton2", MouseButton.Forward],
  ["mouseforward", MouseButton.Forward],
]);

const modifierAliases = new Map<string, Modifier>([
  ["ctrl", Modifier.Control],
  ["control", Modifier.Control],
  ["shift", Modifier.Shift],
  ["alt", Modifier.Alt],
  ["option", Modifier.Alt],
  ["meta", Modifier.Meta],
  ["cmd", Modifier.Meta],
  ["command", Modifier.Meta],
  ["win", Modifier.Meta],
  ["super", Modifier.Meta],
]);

/** Parses portable chords such as `Ctrl+Shift+K`, `Cmd+Space`, or `Alt+LButton`. */
export function parseHotkey(value: string): ParsedHotkey {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError("hotkey must be a non-empty string");
  }
  const tokens = value.split("+").map((token) => token.trim());
  if (tokens.some((token) => token.length === 0)) {
    throw new SyntaxError(`invalid hotkey ${JSON.stringify(value)}: empty chord token`);
  }

  let modifiers = 0;
  let trigger: Omit<ParsedHotkey, "modifiers"> | undefined;
  for (const token of tokens) {
    const normalized = normalize(token);
    const modifier = modifierAliases.get(normalized);
    if (modifier !== undefined) {
      if ((modifiers & modifier) !== 0) {
        throw new SyntaxError(`invalid hotkey ${JSON.stringify(value)}: duplicate ${token}`);
      }
      modifiers |= modifier;
      continue;
    }
    if (trigger) {
      throw new SyntaxError(`invalid hotkey ${JSON.stringify(value)}: multiple trigger keys`);
    }
    const mouse = mouseAliases.get(normalized);
    if (mouse !== undefined) {
      trigger = { device: "mouse", code: mouse };
      continue;
    }
    const key = resolveKey(token, normalized);
    if (key === undefined) {
      throw new SyntaxError(`invalid hotkey ${JSON.stringify(value)}: unknown key ${token}`);
    }
    trigger = { device: "keyboard", code: key };
  }
  if (!trigger) {
    throw new SyntaxError(`invalid hotkey ${JSON.stringify(value)}: missing trigger key`);
  }
  return { ...trigger, modifiers };
}

function resolveKey(token: string, normalized: string): number | undefined {
  if (/^[a-z]$/i.test(token)) return Key[token.toUpperCase() as keyof typeof Key] as number;
  if (/^[0-9]$/.test(token)) {
    return Key[`Digit${token}` as keyof typeof Key] as number;
  }
  return keyAliases.get(normalized) ?? keyNames.get(normalized);
}

function normalize(value: string): string {
  return value.toLowerCase().replaceAll(/[-_\s]/g, "");
}
