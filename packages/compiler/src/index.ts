export {
  compileSource,
  RuneCompileError,
  type CompileDiagnostic,
  type CompileOptions,
  type CompileResult,
} from "./compiler";
export { encodeModule } from "./encode";
export {
  InputDevice,
  InputEdge,
  Opcode,
  SourceFilter,
  WIRE_HANDLER_SIZE,
  WIRE_HEADER_SIZE,
  WIRE_INSTRUCTION_SIZE,
  WIRE_VERSION,
  type CompiledModule,
  type Handler,
  type Instruction,
  type StateSlot,
} from "./ir";
