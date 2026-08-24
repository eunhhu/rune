import {
  WIRE_HANDLER_SIZE,
  WIRE_HEADER_SIZE,
  WIRE_INSTRUCTION_SIZE,
  WIRE_VERSION,
  type CompiledModule,
} from "./ir";

export function encodeModule(module: CompiledModule): Uint8Array {
  const size =
    WIRE_HEADER_SIZE +
    module.states.length * 8 +
    module.handlers.length * WIRE_HANDLER_SIZE +
    module.code.length * WIRE_INSTRUCTION_SIZE;
  const bytes = new Uint8Array(size);
  const view = new DataView(bytes.buffer);
  let offset = 0;

  bytes.set([0x53, 0x50, 0x57, 0x52], offset); // SPWR
  offset += 4;
  view.setUint16(offset, WIRE_VERSION, true);
  offset += 2;
  view.setUint16(offset, 0, true);
  offset += 2;
  view.setUint16(offset, module.states.length, true);
  offset += 2;
  view.setUint16(offset, module.handlers.length, true);
  offset += 2;
  view.setUint16(offset, module.localCount, true);
  offset += 2;
  view.setUint16(offset, module.stackLimit, true);
  offset += 2;
  view.setUint32(offset, module.code.length, true);
  offset += 4;
  view.setUint32(offset, module.instructionBudget, true);
  offset += 4;

  for (const state of module.states) {
    view.setBigInt64(offset, state.initial, true);
    offset += 8;
  }

  for (const handler of module.handlers) {
    view.setUint8(offset, handler.device);
    view.setUint8(offset + 1, handler.edge);
    view.setUint8(offset + 2, handler.source);
    view.setUint8(offset + 3, handler.flags);
    view.setUint16(offset + 4, handler.code, true);
    view.setUint8(offset + 6, handler.modifiers);
    view.setUint8(offset + 7, 0);
    view.setUint32(offset + 8, handler.entry, true);
    view.setUint16(offset + 12, handler.gate, true);
    view.setUint16(offset + 14, 0, true);
    offset += WIRE_HANDLER_SIZE;
  }

  for (const op of module.code) {
    view.setUint8(offset, op.opcode);
    view.setUint8(offset + 1, op.flags);
    view.setUint16(offset + 2, op.a, true);
    view.setUint32(offset + 4, op.b, true);
    view.setBigInt64(offset + 8, op.immediate, true);
    offset += WIRE_INSTRUCTION_SIZE;
  }

  return bytes;
}
