export const WIRE_VERSION = 2;
export const WIRE_HEADER_SIZE = 24;
export const WIRE_HANDLER_SIZE = 12;
export const WIRE_INSTRUCTION_SIZE = 16;

export enum Opcode {
  Halt = 0,
  PushConst = 1,
  LoadState = 2,
  StoreState = 3,
  LoadLocal = 4,
  StoreLocal = 5,
  Pop = 6,
  Dup = 7,
  Add = 8,
  Sub = 9,
  Mul = 10,
  Div = 11,
  Mod = 12,
  Neg = 13,
  Eq = 14,
  Ne = 15,
  Lt = 16,
  Le = 17,
  Gt = 18,
  Ge = 19,
  Not = 20,
  BitAnd = 21,
  BitOr = 22,
  BitXor = 23,
  Shl = 24,
  Shr = 25,
  Jump = 26,
  JumpIfFalse = 27,
  LoadInputCode = 28,
  LoadInputEdge = 29,
  LoadInputSource = 30,
  LoadHeld = 31,
  KeyDown = 32,
  KeyUp = 33,
  MouseDown = 34,
  MouseUp = 35,
  MouseMove = 36,
  MouseWheel = 37,
  DelayUs = 38,
}

export enum InputDevice {
  Keyboard = 0,
  MouseButton = 1,
}

export enum InputEdge {
  Down = 0,
  Up = 1,
}

export enum SourceFilter {
  Physical = 0,
  Synthetic = 1,
  Any = 2,
}

export interface Instruction {
  opcode: Opcode;
  flags: number;
  a: number;
  b: number;
  immediate: bigint;
}

export interface Handler {
  device: InputDevice;
  edge: InputEdge;
  source: SourceFilter;
  code: number;
  entry: number;
}

export interface StateSlot {
  name: string;
  slot: number;
  kind: "number" | "boolean";
  initial: bigint;
}

export interface CompiledModule {
  states: StateSlot[];
  handlers: Handler[];
  code: Instruction[];
  localCount: number;
  stackLimit: number;
  instructionBudget: number;
}

export function instruction(opcode: Opcode): Instruction {
  return { opcode, flags: 0, a: 0, b: 0, immediate: 0n };
}
