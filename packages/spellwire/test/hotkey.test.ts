import { describe, expect, test } from "bun:test";

import { Key, Modifier, MouseButton, parseHotkey } from "../src/index";

describe("portable hotkey parser", () => {
  test("parses logical modifiers, aliases, keyboard keys, and mouse buttons", () => {
    expect(parseHotkey("Ctrl+Shift+K")).toEqual({
      device: "keyboard",
      code: Key.K,
      modifiers: Modifier.Control | Modifier.Shift,
    });
    expect(parseHotkey("Cmd + Space")).toEqual({
      device: "keyboard",
      code: Key.Space,
      modifiers: Modifier.Meta,
    });
    expect(parseHotkey("Alt+LButton")).toEqual({
      device: "mouse",
      code: MouseButton.Left,
      modifiers: Modifier.Alt,
    });
  });

  test("rejects ambiguous chords", () => {
    expect(() => parseHotkey("Ctrl")).toThrow("missing trigger key");
    expect(() => parseHotkey("Ctrl+K+L")).toThrow("multiple trigger keys");
    expect(() => parseHotkey("Ctrl+Ctrl+K")).toThrow("duplicate");
    expect(() => parseHotkey("Ctrl+NoSuchKey")).toThrow("unknown key");
  });
});
