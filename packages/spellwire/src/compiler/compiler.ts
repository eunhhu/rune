import ts from "typescript";
import { InputSource, Key, Modifier, MouseButton } from "../keys";
import { parseHotkey } from "../hotkey";
import {
  InputDevice,
  InputEdge,
  NO_STATE_GATE,
  Opcode,
  SourceFilter,
  TriggerFlag,
  instruction,
  type CompiledModule,
  type Handler,
  type Instruction,
  type StateSlot,
} from "./ir";

export interface CompileOptions {
  fileName?: string;
  stackLimit?: number;
  instructionBudget?: number;
}

export interface CompileDiagnostic {
  fileName: string;
  line: number;
  column: number;
  message: string;
}

export class SpellwireCompileError extends Error {
  constructor(readonly diagnostics: readonly CompileDiagnostic[]) {
    super(diagnostics.map(formatDiagnostic).join("\n"));
    this.name = "SpellwireCompileError";
  }
}

export interface CompileResult {
  module: CompiledModule;
  sourceFile: ts.SourceFile;
}

type Binding = { kind: "state" | "local"; slot: number };

type FunctionDeclarationWithBody = ts.FunctionDeclaration & { body: ts.Block; name: ts.Identifier };

type SourceFileWithParseDiagnostics = ts.SourceFile & {
  readonly parseDiagnostics?: readonly ts.Diagnostic[];
};

const MAX_TRIGGER_KEY_CODE = 0xff;
const MAX_TRIGGER_MOUSE_BUTTON = 7;

interface LoopContext {
  breakPatches: number[];
  continuePatches: number[];
}

interface FunctionContext {
  returnPatches: number[];
}

interface ParsedTriggerOptions {
  source: SourceFilter;
  flags: number;
  modifiers: number;
  gate: number;
  edge?: InputEdge;
}

class Scope {
  readonly bindings = new Map<string, Binding>();

  constructor(readonly parent?: Scope) {}

  get(name: string): Binding | undefined {
    return this.bindings.get(name) ?? this.parent?.get(name);
  }
}

class Compiler {
  readonly sourceFile: ts.SourceFile;
  readonly states: StateSlot[] = [];
  readonly stateBindings = new Map<string, Binding>();
  readonly constants = new Map<string, ts.Expression>();
  readonly functions = new Map<string, FunctionDeclarationWithBody>();
  readonly handlers: Handler[] = [];
  readonly code: Instruction[] = [];
  readonly loopStack: LoopContext[] = [];
  readonly functionStack: FunctionContext[] = [];
  readonly inlineStack: string[] = [];

  #nextLocal = 0;
  #maxLocal = 0;

  constructor(source: string, readonly options: Required<CompileOptions>) {
    this.sourceFile = ts.createSourceFile(
      options.fileName,
      source,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TS,
    );
  }

  compile(): CompileResult {
    this.rejectSyntaxErrors();
    this.collectTopLevelDeclarations();
    this.collectHandlers();
    if (this.handlers.length === 0) {
      this.fail(this.sourceFile, "No rt.hotkey/remap/onKey*/onMouse* handlers were found");
    }
    return {
      sourceFile: this.sourceFile,
      module: {
        states: this.states,
        handlers: this.handlers,
        code: this.code,
        localCount: this.#maxLocal,
        stackLimit: this.options.stackLimit,
        instructionBudget: this.options.instructionBudget,
      },
    };
  }

  collectTopLevelDeclarations(): void {
    for (const statement of this.sourceFile.statements) {
      if (ts.isVariableStatement(statement)) {
        const isConst = (statement.declarationList.flags & ts.NodeFlags.Const) !== 0;
        for (const declaration of statement.declarationList.declarations) {
          if (!ts.isIdentifier(declaration.name) || !declaration.initializer) continue;
          const name = declaration.name.text;
          if (isConst) {
            this.constants.set(name, declaration.initializer);
            continue;
          }
          const initial = this.constantValue(declaration.initializer, new Set());
          // Spellwire modules may contain arbitrary control-plane TypeScript next to realtime
          // handlers. Only top-level `let` declarations that can be represented by the
          // native integer VM are captured as persistent realtime state. Unsupported
          // declarations stay in Bun and produce an error only if a realtime handler
          // actually references them.
          if (initial === undefined) continue;
          const kind =
            declaration.initializer.kind === ts.SyntaxKind.TrueKeyword ||
            declaration.initializer.kind === ts.SyntaxKind.FalseKeyword
              ? "boolean"
              : "number";
          const slot = this.states.length;
          this.states.push({ name, slot, kind, initial });
          this.stateBindings.set(name, { kind: "state", slot });
        }
      } else if (
        ts.isFunctionDeclaration(statement) &&
        statement.name &&
        statement.body
      ) {
        this.functions.set(statement.name.text, statement as FunctionDeclarationWithBody);
      }
    }
  }

  collectHandlers(): void {
    for (const statement of this.sourceFile.statements) {
      if (!ts.isExpressionStatement(statement) || !ts.isCallExpression(statement.expression)) {
        continue;
      }
      const call = statement.expression;
      if (!ts.isPropertyAccessExpression(call.expression)) continue;
      const target = call.expression.expression;
      if (!ts.isIdentifier(target) || target.text !== "rt") continue;

      const method = call.expression.name.text;
      if (method === "remap") {
        this.collectRemap(call);
        continue;
      }
      const trigger = this.parseTrigger(method, call);
      if (!trigger) continue;
      const callback = call.arguments[1];
      if (!callback || (!ts.isArrowFunction(callback) && !ts.isFunctionExpression(callback))) {
        this.fail(call, `${method} requires an inline arrow/function callback`);
      }

      const entry = this.code.length;
      this.#nextLocal = 0;
      const scope = new Scope();
      const context: FunctionContext = { returnPatches: [] };
      this.functionStack.push(context);
      if (ts.isBlock(callback.body)) {
        this.compileBlock(callback.body, scope);
      } else {
        const hasValue = this.compileExpression(callback.body, scope);
        if (hasValue) this.emit(Opcode.Pop);
      }
      this.patchAll(context.returnPatches, this.code.length);
      this.functionStack.pop();
      this.emit(Opcode.Halt);
      this.handlers.push({ ...trigger, entry });
    }
  }

  parseTrigger(method: string, call: ts.CallExpression): Omit<Handler, "entry"> | undefined {
    let device: InputDevice;
    let edge: InputEdge;
    let code: number;
    let optionDefaults: Partial<ParsedTriggerOptions> = {};
    let allowModifierOption = true;
    switch (method) {
      case "onKeyDown":
        device = InputDevice.Keyboard;
        edge = InputEdge.Down;
        break;
      case "onKeyUp":
        device = InputDevice.Keyboard;
        edge = InputEdge.Up;
        break;
      case "onMouseDown":
        device = InputDevice.MouseButton;
        edge = InputEdge.Down;
        break;
      case "onMouseUp":
        device = InputDevice.MouseButton;
        edge = InputEdge.Up;
        break;
      case "hotkey": {
        const chordExpression = call.arguments[0];
        if (!chordExpression) this.fail(call, "hotkey requires a chord string");
        const chord = this.constantString(chordExpression);
        if (chord === undefined) {
          this.fail(chordExpression, "hotkey chord must be a constant string");
        }
        let parsed: ReturnType<typeof parseHotkey>;
        try {
          parsed = parseHotkey(chord);
        } catch (error) {
          this.fail(chordExpression, error instanceof Error ? error.message : String(error));
        }
        device = parsed.device === "keyboard" ? InputDevice.Keyboard : InputDevice.MouseButton;
        edge = InputEdge.Down;
        code = parsed.code;
        optionDefaults = {
          flags: TriggerFlag.Consume | TriggerFlag.ExactModifiers,
          modifiers: parsed.modifiers,
        };
        allowModifierOption = false;
        const options = this.parseTriggerOptions(
          call.arguments[2],
          optionDefaults,
          allowModifierOption,
          ["source", "consume", "exactModifiers", "repeat", "when", "edge"],
        );
        return { device, edge: options.edge ?? edge, code, ...options };
      }
      default:
        return undefined;
    }
    const codeExpression = call.arguments[0];
    if (!codeExpression) this.fail(call, `${method} requires a key/button argument`);
    code = this.constantNumber(codeExpression) ?? -1;
    if (code === -1) {
      this.fail(codeExpression, `${method} key/button must be a constant integer`);
    }
    const maxCode =
      device === InputDevice.Keyboard ? MAX_TRIGGER_KEY_CODE : MAX_TRIGGER_MOUSE_BUTTON;
    if (code < 0 || code > maxCode) {
      const label = device === InputDevice.Keyboard ? "key code" : "mouse button";
      this.fail(codeExpression, `${method} ${label} must be between 0 and ${maxCode}`);
    }
    const options = this.parseTriggerOptions(
      call.arguments[2],
      optionDefaults,
      allowModifierOption,
    );
    return { device, edge, code, ...options };
  }

  parseTriggerOptions(
    options: ts.Expression | undefined,
    defaults: Partial<ParsedTriggerOptions> = {},
    allowModifierOption = true,
    allowedOptions: readonly string[] = [
      "source",
      "consume",
      "modifiers",
      "exactModifiers",
      "repeat",
      "when",
    ],
  ): ParsedTriggerOptions {
    let source = defaults.source ?? SourceFilter.Physical;
    let flags = defaults.flags ?? 0;
    let modifiers = defaults.modifiers ?? 0;
    let gate = defaults.gate ?? NO_STATE_GATE;
    let edge = defaults.edge;
    if (!options) return { source, flags, modifiers, gate, ...(edge === undefined ? {} : { edge }) };
    if (!ts.isObjectLiteralExpression(options)) {
      this.fail(options, "Realtime handler options must be an object literal");
    }
    const seen = new Set<string>();
    for (const property of options.properties) {
      let valueExpression: ts.Expression;
      let name: string | undefined;
      if (ts.isPropertyAssignment(property)) {
        name = this.propertyName(property.name);
        valueExpression = property.initializer;
      } else if (ts.isShorthandPropertyAssignment(property)) {
        name = property.name.text;
        valueExpression = property.name;
      } else {
        this.fail(
          property,
          "Realtime handler options require plain properties",
        );
      }
      if (!name || !allowedOptions.includes(name)) {
        this.fail(property, `Unsupported realtime handler option ${JSON.stringify(name)}`);
      }
      if (name === "modifiers" && !allowModifierOption) {
        this.fail(property, "hotkey modifiers belong in the chord string");
      }
      if (seen.has(name)) this.fail(property, `Duplicate realtime handler option ${name}`);
      seen.add(name);
      if (name === "when") {
        const parsed = this.parseStateGate(valueExpression);
        gate = parsed.gate;
        flags = parsed.inverted
          ? flags | TriggerFlag.GateInverted
          : flags & ~TriggerFlag.GateInverted;
        continue;
      }
      if (name === "edge") {
        const value = this.constantString(valueExpression);
        if (value === "down") edge = InputEdge.Down;
        else if (value === "up") edge = InputEdge.Up;
        else this.fail(valueExpression, "edge must be \"down\" or \"up\"");
        continue;
      }
      const value = this.constantNumber(valueExpression);
      if (value === undefined) this.fail(valueExpression, `${name} must be a compile-time constant`);
      switch (name) {
        case "source":
          if (value === InputSource.Physical) source = SourceFilter.Physical;
          else if (value === InputSource.Synthetic) source = SourceFilter.Synthetic;
          else if (value === InputSource.Any) source = SourceFilter.Any;
          else this.fail(valueExpression, "Invalid InputSource value");
          break;
        case "consume":
          flags = this.withBooleanFlag(flags, TriggerFlag.Consume, value, valueExpression);
          break;
        case "exactModifiers":
          flags = this.withBooleanFlag(
            flags,
            TriggerFlag.ExactModifiers,
            value,
            valueExpression,
          );
          break;
        case "repeat":
          flags = this.withBooleanFlag(flags, TriggerFlag.IgnoreRepeat, 1 - value, valueExpression);
          break;
        case "modifiers":
          if (value < 0 || value > 0x0f) {
            this.fail(valueExpression, "modifiers must use the four logical Modifier bits");
          }
          modifiers = value;
          break;
      }
    }
    return { source, flags, modifiers, gate, ...(edge === undefined ? {} : { edge }) };
  }

  parseStateGate(expression: ts.Expression): { gate: number; inverted: boolean } {
    if (!ts.isArrowFunction(expression) && !ts.isFunctionExpression(expression)) {
      this.fail(expression, "when must be a zero-argument function returning native boolean state");
    }
    if (expression.parameters.length !== 0) {
      this.fail(expression, "when gate cannot declare parameters");
    }
    let result: ts.Expression;
    if (ts.isBlock(expression.body)) {
      const [statement] = expression.body.statements;
      if (
        expression.body.statements.length !== 1 ||
        !statement ||
        !ts.isReturnStatement(statement) ||
        !statement.expression
      ) {
        this.fail(expression.body, "when gate body must contain one return expression");
      }
      result = statement.expression;
    } else {
      result = expression.body;
    }
    while (
      ts.isParenthesizedExpression(result) ||
      ts.isAsExpression(result) ||
      ts.isTypeAssertionExpression(result) ||
      ts.isNonNullExpression(result)
    ) {
      result = result.expression;
    }
    let inverted = false;
    if (ts.isPrefixUnaryExpression(result) && result.operator === ts.SyntaxKind.ExclamationToken) {
      inverted = true;
      result = result.operand;
    }
    if (!ts.isIdentifier(result)) {
      this.fail(result, "when gate must return a native boolean state or its negation");
    }
    const binding = this.stateBindings.get(result.text);
    const state = binding?.kind === "state" ? this.states[binding.slot] : undefined;
    if (!state || state.kind !== "boolean") {
      this.fail(result, "when gate must reference a module-scope boolean let state");
    }
    if (state.slot >= NO_STATE_GATE) {
      this.fail(result, `when gates support the first ${NO_STATE_GATE} native states`);
    }
    return { gate: state.slot, inverted };
  }

  withBooleanFlag(flags: number, flag: number, value: number, node: ts.Node): number {
    if (value !== 0 && value !== 1) this.fail(node, "Boolean option must be true or false");
    return value === 1 ? flags | flag : flags & ~flag;
  }

  collectRemap(call: ts.CallExpression): void {
    const fromExpression = call.arguments[0];
    const toExpression = call.arguments[1];
    if (!fromExpression || !toExpression) this.fail(call, "remap requires source and target keys");
    const from = this.constantRemapKey(fromExpression, "source", MAX_TRIGGER_KEY_CODE);
    const to = this.constantRemapKey(toExpression, "target", MAX_TRIGGER_KEY_CODE);
    const options = this.parseTriggerOptions(
      call.arguments[2],
      { flags: TriggerFlag.Consume },
      false,
      ["source", "repeat", "when"],
    );
    if (options.modifiers !== 0 || (options.flags & TriggerFlag.ExactModifiers) !== 0) {
      this.fail(call.arguments[2] ?? call, "remap options support only source, repeat, and when");
    }
    for (const [edge, opcode] of [
      [InputEdge.Down, Opcode.KeyDown],
      [InputEdge.Up, Opcode.KeyUp],
    ] as const) {
      const entry = this.code.length;
      this.emit(opcode, { a: to });
      this.emit(Opcode.Halt);
      this.handlers.push({
        device: InputDevice.Keyboard,
        edge,
        source: options.source,
        flags: options.flags | TriggerFlag.Consume,
        modifiers: 0,
        gate: options.gate,
        code: from,
        entry,
      });
    }
  }

  constantRemapKey(expression: ts.Expression, label: "source" | "target", maximum: number): number {
    let value = this.constantNumber(expression);
    if (value === undefined) {
      const name = this.constantString(expression);
      if (name !== undefined) {
        try {
          const parsed = parseHotkey(name);
          if (parsed.device === "keyboard" && parsed.modifiers === 0) value = parsed.code;
        } catch {
          // The uniform diagnostic below is clearer for remap call sites.
        }
      }
    }
    if (value === undefined || value < 0 || value > maximum) {
      this.fail(expression, `remap ${label} must be one constant keyboard key`);
    }
    return value;
  }

  propertyName(name: ts.PropertyName): string | undefined {
    if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
      return name.text;
    }
    if (ts.isComputedPropertyName(name) && ts.isStringLiteral(name.expression)) {
      return name.expression.text;
    }
    return undefined;
  }

  compileBlock(block: ts.Block, parent: Scope): void {
    const scope = new Scope(parent);
    for (const statement of block.statements) this.compileStatement(statement, scope);
  }

  compileStatement(statement: ts.Statement, scope: Scope): void {
    if (ts.isBlock(statement)) {
      this.compileBlock(statement, scope);
      return;
    }
    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        if (!ts.isIdentifier(declaration.name)) {
          this.fail(declaration.name, "Destructuring is not supported in realtime handlers yet");
        }
        const binding = this.allocateLocal(declaration.name.text, scope);
        if (declaration.initializer) {
          if (!this.compileExpression(declaration.initializer, scope)) {
            this.fail(declaration.initializer, "Variable initializer must produce a value");
          }
        } else {
          this.emit(Opcode.PushConst, { immediate: 0n });
        }
        this.emit(Opcode.StoreLocal, { a: binding.slot });
      }
      return;
    }
    if (ts.isExpressionStatement(statement)) {
      if (this.compileExpression(statement.expression, scope)) this.emit(Opcode.Pop);
      return;
    }
    if (ts.isIfStatement(statement)) {
      this.requireValue(statement.expression, scope);
      const falseJump = this.emit(Opcode.JumpIfFalse);
      this.compileStatement(statement.thenStatement, scope);
      if (statement.elseStatement) {
        const endJump = this.emit(Opcode.Jump);
        this.patch(falseJump, this.code.length);
        this.compileStatement(statement.elseStatement, scope);
        this.patch(endJump, this.code.length);
      } else {
        this.patch(falseJump, this.code.length);
      }
      return;
    }
    if (ts.isForStatement(statement)) {
      this.compileFor(statement, scope);
      return;
    }
    if (ts.isWhileStatement(statement)) {
      this.compileWhile(statement, scope);
      return;
    }
    if (ts.isDoStatement(statement)) {
      this.compileDoWhile(statement, scope);
      return;
    }
    if (ts.isBreakStatement(statement)) {
      const loop = this.loopStack.at(-1);
      if (!loop) this.fail(statement, "break is only valid inside a realtime loop");
      loop.breakPatches.push(this.emit(Opcode.Jump));
      return;
    }
    if (ts.isContinueStatement(statement)) {
      const loop = this.loopStack.at(-1);
      if (!loop) this.fail(statement, "continue is only valid inside a realtime loop");
      loop.continuePatches.push(this.emit(Opcode.Jump));
      return;
    }
    if (ts.isReturnStatement(statement)) {
      if (statement.expression) {
        this.fail(statement.expression, "Realtime helper functions currently support void return only");
      }
      const fn = this.functionStack.at(-1);
      if (!fn) this.fail(statement, "return is not valid here");
      fn.returnPatches.push(this.emit(Opcode.Jump));
      return;
    }
    if (ts.isEmptyStatement(statement)) return;
    this.fail(statement, `Unsupported realtime statement: ${ts.SyntaxKind[statement.kind]}`);
  }

  compileFor(statement: ts.ForStatement, parent: Scope): void {
    const scope = new Scope(parent);
    if (statement.initializer) {
      if (ts.isVariableDeclarationList(statement.initializer)) {
        for (const declaration of statement.initializer.declarations) {
          if (!ts.isIdentifier(declaration.name)) {
            this.fail(declaration.name, "for-loop destructuring is not supported");
          }
          const binding = this.allocateLocal(declaration.name.text, scope);
          if (declaration.initializer) {
            this.requireValue(declaration.initializer, scope);
          } else {
            this.emit(Opcode.PushConst, { immediate: 0n });
          }
          this.emit(Opcode.StoreLocal, { a: binding.slot });
        }
      } else if (this.compileExpression(statement.initializer, scope)) {
        this.emit(Opcode.Pop);
      }
    }

    const conditionPc = this.code.length;
    let endCondition: number | undefined;
    if (statement.condition) {
      this.requireValue(statement.condition, scope);
      endCondition = this.emit(Opcode.JumpIfFalse);
    }

    const loop: LoopContext = { breakPatches: [], continuePatches: [] };
    this.loopStack.push(loop);
    this.compileStatement(statement.statement, scope);
    this.loopStack.pop();

    const incrementPc = this.code.length;
    this.patchAll(loop.continuePatches, incrementPc);
    if (statement.incrementor && this.compileExpression(statement.incrementor, scope)) {
      this.emit(Opcode.Pop);
    }
    this.emit(Opcode.Jump, { b: conditionPc });
    const endPc = this.code.length;
    if (endCondition !== undefined) this.patch(endCondition, endPc);
    this.patchAll(loop.breakPatches, endPc);
  }

  compileWhile(statement: ts.WhileStatement, scope: Scope): void {
    const conditionPc = this.code.length;
    this.requireValue(statement.expression, scope);
    const endCondition = this.emit(Opcode.JumpIfFalse);
    const loop: LoopContext = { breakPatches: [], continuePatches: [] };
    this.loopStack.push(loop);
    this.compileStatement(statement.statement, scope);
    this.loopStack.pop();
    this.patchAll(loop.continuePatches, conditionPc);
    this.emit(Opcode.Jump, { b: conditionPc });
    const endPc = this.code.length;
    this.patch(endCondition, endPc);
    this.patchAll(loop.breakPatches, endPc);
  }

  compileDoWhile(statement: ts.DoStatement, scope: Scope): void {
    const bodyPc = this.code.length;
    const loop: LoopContext = { breakPatches: [], continuePatches: [] };
    this.loopStack.push(loop);
    this.compileStatement(statement.statement, scope);
    this.loopStack.pop();
    const conditionPc = this.code.length;
    this.patchAll(loop.continuePatches, conditionPc);
    this.requireValue(statement.expression, scope);
    this.emit(Opcode.Not);
    this.emit(Opcode.JumpIfFalse, { b: bodyPc });
    const endPc = this.code.length;
    this.patchAll(loop.breakPatches, endPc);
  }

  compileExpression(expression: ts.Expression, scope: Scope): boolean {
    if (
      ts.isParenthesizedExpression(expression) ||
      ts.isAsExpression(expression) ||
      ts.isTypeAssertionExpression(expression) ||
      ts.isNonNullExpression(expression)
    ) {
      return this.compileExpression(expression.expression, scope);
    }
    if (ts.isNumericLiteral(expression)) {
      const value = Number(expression.text);
      if (!Number.isSafeInteger(value)) this.fail(expression, "Realtime numbers must be safe integers");
      this.emit(Opcode.PushConst, { immediate: BigInt(value) });
      return true;
    }
    if (expression.kind === ts.SyntaxKind.TrueKeyword) {
      this.emit(Opcode.PushConst, { immediate: 1n });
      return true;
    }
    if (expression.kind === ts.SyntaxKind.FalseKeyword) {
      this.emit(Opcode.PushConst, { immediate: 0n });
      return true;
    }
    if (ts.isIdentifier(expression)) {
      const binding = scope.get(expression.text) ?? this.stateBindings.get(expression.text);
      if (binding) {
        this.emit(binding.kind === "state" ? Opcode.LoadState : Opcode.LoadLocal, {
          a: binding.slot,
        });
        return true;
      }
      const constant = this.constantValue(expression, new Set());
      if (constant !== undefined) {
        this.emit(Opcode.PushConst, { immediate: constant });
        return true;
      }
      this.fail(expression, `Unknown realtime identifier ${expression.text}`);
    }
    if (ts.isPropertyAccessExpression(expression)) {
      const constant = this.constantValue(expression, new Set());
      if (constant !== undefined) {
        this.emit(Opcode.PushConst, { immediate: constant });
        return true;
      }
      this.fail(expression, `Unsupported realtime property access ${expression.getText(this.sourceFile)}`);
    }
    if (ts.isPrefixUnaryExpression(expression)) {
      if (
        expression.operator === ts.SyntaxKind.PlusPlusToken ||
        expression.operator === ts.SyntaxKind.MinusMinusToken
      ) {
        return this.compileUpdate(
          expression.operand,
          expression.operator === ts.SyntaxKind.PlusPlusToken ? 1n : -1n,
          true,
          scope,
        );
      }
      this.requireValue(expression.operand, scope);
      switch (expression.operator) {
        case ts.SyntaxKind.ExclamationToken:
          this.emit(Opcode.Not);
          return true;
        case ts.SyntaxKind.MinusToken:
          this.emit(Opcode.Neg);
          return true;
        case ts.SyntaxKind.PlusToken:
          return true;
        case ts.SyntaxKind.TildeToken:
          this.emit(Opcode.PushConst, { immediate: -1n });
          this.emit(Opcode.BitXor);
          return true;
        default:
          this.fail(expression, "Unsupported realtime unary operator");
      }
    }
    if (ts.isPostfixUnaryExpression(expression)) {
      return this.compileUpdate(
        expression.operand,
        expression.operator === ts.SyntaxKind.PlusPlusToken ? 1n : -1n,
        false,
        scope,
      );
    }
    if (ts.isBinaryExpression(expression)) {
      return this.compileBinary(expression, scope);
    }
    if (ts.isConditionalExpression(expression)) {
      this.requireValue(expression.condition, scope);
      const falseJump = this.emit(Opcode.JumpIfFalse);
      this.requireValue(expression.whenTrue, scope);
      const endJump = this.emit(Opcode.Jump);
      this.patch(falseJump, this.code.length);
      this.requireValue(expression.whenFalse, scope);
      this.patch(endJump, this.code.length);
      return true;
    }
    if (ts.isCallExpression(expression)) {
      return this.compileCall(expression, scope);
    }
    this.fail(expression, `Unsupported realtime expression: ${ts.SyntaxKind[expression.kind]}`);
  }

  compileBinary(expression: ts.BinaryExpression, scope: Scope): boolean {
    const operator = expression.operatorToken.kind;
    if (
      operator === ts.SyntaxKind.EqualsToken ||
      operator === ts.SyntaxKind.PlusEqualsToken ||
      operator === ts.SyntaxKind.MinusEqualsToken ||
      operator === ts.SyntaxKind.AsteriskEqualsToken ||
      operator === ts.SyntaxKind.SlashEqualsToken ||
      operator === ts.SyntaxKind.PercentEqualsToken ||
      operator === ts.SyntaxKind.AmpersandEqualsToken ||
      operator === ts.SyntaxKind.BarEqualsToken ||
      operator === ts.SyntaxKind.CaretEqualsToken ||
      operator === ts.SyntaxKind.LessThanLessThanEqualsToken ||
      operator === ts.SyntaxKind.GreaterThanGreaterThanEqualsToken
    ) {
      return this.compileAssignment(expression.left, operator, expression.right, scope);
    }
    if (operator === ts.SyntaxKind.AmpersandAmpersandToken) {
      this.requireValue(expression.left, scope);
      this.emit(Opcode.Dup);
      const endJump = this.emit(Opcode.JumpIfFalse);
      this.emit(Opcode.Pop);
      this.requireValue(expression.right, scope);
      this.patch(endJump, this.code.length);
      return true;
    }
    if (operator === ts.SyntaxKind.BarBarToken) {
      this.requireValue(expression.left, scope);
      this.emit(Opcode.Dup);
      this.emit(Opcode.Not);
      const endJump = this.emit(Opcode.JumpIfFalse);
      this.emit(Opcode.Pop);
      this.requireValue(expression.right, scope);
      this.patch(endJump, this.code.length);
      return true;
    }

    this.requireValue(expression.left, scope);
    this.requireValue(expression.right, scope);
    const opcode = binaryOpcode(operator);
    if (opcode === undefined) {
      this.fail(expression.operatorToken, "Unsupported realtime binary operator");
    }
    this.emit(opcode);
    return true;
  }

  compileAssignment(
    target: ts.Expression,
    operator: ts.SyntaxKind,
    value: ts.Expression,
    scope: Scope,
  ): true {
    const binding = this.resolveWritable(target, scope);
    if (operator !== ts.SyntaxKind.EqualsToken) {
      this.emit(binding.kind === "state" ? Opcode.LoadState : Opcode.LoadLocal, {
        a: binding.slot,
      });
    }
    this.requireValue(value, scope);
    if (operator !== ts.SyntaxKind.EqualsToken) {
      const opcode = compoundOpcode(operator);
      if (opcode === undefined) this.fail(target, "Unsupported realtime compound assignment");
      this.emit(opcode);
    }
    this.emit(Opcode.Dup);
    this.emit(binding.kind === "state" ? Opcode.StoreState : Opcode.StoreLocal, {
      a: binding.slot,
    });
    return true;
  }

  compileUpdate(
    target: ts.Expression,
    delta: bigint,
    prefix: boolean,
    scope: Scope,
  ): true {
    const binding = this.resolveWritable(target, scope);
    const load = binding.kind === "state" ? Opcode.LoadState : Opcode.LoadLocal;
    const store = binding.kind === "state" ? Opcode.StoreState : Opcode.StoreLocal;
    this.emit(load, { a: binding.slot });
    if (!prefix) this.emit(Opcode.Dup);
    this.emit(Opcode.PushConst, { immediate: delta < 0n ? -delta : delta });
    this.emit(delta < 0n ? Opcode.Sub : Opcode.Add);
    if (prefix) this.emit(Opcode.Dup);
    this.emit(store, { a: binding.slot });
    return true;
  }

  compileCall(call: ts.CallExpression, scope: Scope): boolean {
    const name = callName(call.expression);
    if (!name) this.fail(call.expression, "Realtime calls must target a named function");

    switch (name) {
      case "keyDown":
        this.emitSingleValueOutput(Opcode.KeyDown, call.arguments[0], call, scope, "key code");
        return false;
      case "keyUp":
        this.emitSingleValueOutput(Opcode.KeyUp, call.arguments[0], call, scope, "key code");
        return false;
      case "tapKey": {
        const argument = call.arguments[0];
        if (!argument) this.fail(call, "tapKey requires a key code");
        const constant = this.constantNumber(argument);
        if (constant !== undefined) {
          this.requireU16Value(constant, argument, "key code");
          this.emit(Opcode.KeyDown, { a: constant });
          this.emit(Opcode.KeyUp, { a: constant });
        } else {
          this.requireValue(argument, scope);
          this.emit(Opcode.Dup);
          this.emit(Opcode.KeyDown, { flags: 0x80 });
          this.emit(Opcode.KeyUp, { flags: 0x80 });
        }
        return false;
      }
      case "mouseDown":
        this.emitSingleValueOutput(Opcode.MouseDown, call.arguments[0], call, scope, "mouse button");
        return false;
      case "mouseUp":
        this.emitSingleValueOutput(Opcode.MouseUp, call.arguments[0], call, scope, "mouse button");
        return false;
      case "clickMouse": {
        const argument = call.arguments[0];
        if (!argument) this.fail(call, "clickMouse requires a mouse button");
        const constant = this.constantNumber(argument);
        if (constant !== undefined) {
          this.requireU16Value(constant, argument, "mouse button");
          this.emit(Opcode.MouseDown, { a: constant });
          this.emit(Opcode.MouseUp, { a: constant });
        } else {
          this.requireValue(argument, scope);
          this.emit(Opcode.Dup);
          this.emit(Opcode.MouseDown, { flags: 0x80 });
          this.emit(Opcode.MouseUp, { flags: 0x80 });
        }
        return false;
      }
      case "moveMouse":
        this.emitPairOutput(Opcode.MouseMove, call.arguments[0], call.arguments[1], call, scope);
        return false;
      case "wheelMouse":
        this.emitPairOutput(Opcode.MouseWheel, call.arguments[0], call.arguments[1], call, scope);
        return false;
      case "sleepUs": {
        const duration = call.arguments[0];
        if (!duration) this.fail(call, "sleepUs requires a duration");
        const constant = this.constantNumber(duration);
        if (constant !== undefined) {
          if (constant < 0 || constant > 0xffff_ffff) {
            this.fail(duration, "sleepUs duration must fit u32");
          }
          this.emit(Opcode.DelayUs, { b: constant });
        } else {
          this.requireValue(duration, scope);
          this.emit(Opcode.DelayUs, { flags: 0x80 });
        }
        return false;
      }
      case "keyHeld":
        this.emitHeld(InputDevice.Keyboard, call.arguments[0], call, scope);
        return true;
      case "mouseHeld":
        this.emitHeld(InputDevice.MouseButton, call.arguments[0], call, scope);
        return true;
      default:
        break;
    }

    const declaration = this.functions.get(name);
    if (!declaration) this.fail(call.expression, `Unsupported realtime function ${name}`);
    this.inlineFunction(declaration, call.arguments, scope);
    return false;
  }

  emitSingleValueOutput(
    opcode: Opcode,
    argument: ts.Expression | undefined,
    owner: ts.Node,
    scope: Scope,
    label: string,
  ): void {
    if (!argument) this.fail(owner, `Missing ${label}`);
    const constant = this.constantNumber(argument);
    if (constant !== undefined) {
      this.requireU16Value(constant, argument, label);
      this.emit(opcode, { a: constant });
      return;
    }
    this.requireValue(argument, scope);
    this.emit(opcode, { flags: 0x80 });
  }

  emitPairOutput(
    opcode: Opcode,
    xExpression: ts.Expression | undefined,
    yExpression: ts.Expression | undefined,
    owner: ts.Node,
    scope: Scope,
  ): void {
    if (!xExpression || !yExpression) this.fail(owner, "Mouse output requires x and y values");
    const x = this.constantNumber(xExpression);
    const y = this.constantNumber(yExpression);
    if (x !== undefined && y !== undefined) {
      this.requireI32Value(x, xExpression);
      this.requireI32Value(y, yExpression);
      this.emit(opcode, { immediate: packPair(x, y) });
      return;
    }
    this.requireValue(xExpression, scope);
    this.requireValue(yExpression, scope);
    this.emit(opcode, { flags: 0x80 });
  }

  emitHeld(
    device: InputDevice,
    argument: ts.Expression | undefined,
    owner: ts.Node,
    scope: Scope,
  ): void {
    if (!argument) this.fail(owner, "Held query requires an input code");
    const constant = this.constantNumber(argument);
    if (constant !== undefined) {
      this.requireU16Value(constant, argument, "input code");
      this.emit(Opcode.LoadHeld, { flags: device, a: constant });
      return;
    }
    this.requireValue(argument, scope);
    // InputDevice currently occupies bit zero, so stack-operands uses bit seven.
    this.emit(Opcode.LoadHeld, { flags: device | 0x80 });
  }

  requireU16Value(value: number, node: ts.Node, label: string): void {
    if (value < 0 || value > 0xffff) this.fail(node, `${label} must fit u16`);
  }

  requireI32Value(value: number, node: ts.Node): void {
    if (value < -0x8000_0000 || value > 0x7fff_ffff) {
      this.fail(node, "Mouse coordinate must fit i32");
    }
  }

  inlineFunction(
    declaration: FunctionDeclarationWithBody,
    argumentsList: ts.NodeArray<ts.Expression>,
    callerScope: Scope,
  ): void {
    const name = declaration.name.text;
    if (this.inlineStack.includes(name)) {
      this.fail(declaration, `Recursive realtime function ${name} is not supported`);
    }
    if (argumentsList.length !== declaration.parameters.length) {
      this.fail(
        declaration,
        `Realtime function ${name} expects ${declaration.parameters.length} arguments, got ${argumentsList.length}`,
      );
    }

    this.inlineStack.push(name);
    const scope = new Scope();
    for (let index = 0; index < declaration.parameters.length; index += 1) {
      const parameter = declaration.parameters[index];
      const argument = argumentsList[index];
      if (!parameter || !argument || !ts.isIdentifier(parameter.name)) {
        this.fail(declaration, `Realtime function ${name} supports identifier parameters only`);
      }
      const binding = this.allocateLocal(parameter.name.text, scope);
      this.requireValue(argument, callerScope);
      this.emit(Opcode.StoreLocal, { a: binding.slot });
    }

    const context: FunctionContext = { returnPatches: [] };
    this.functionStack.push(context);
    this.compileBlock(declaration.body, scope);
    this.patchAll(context.returnPatches, this.code.length);
    this.functionStack.pop();
    this.inlineStack.pop();
  }

  resolveWritable(target: ts.Expression, scope: Scope): Binding {
    if (!ts.isIdentifier(target)) {
      this.fail(target, "Realtime assignment target must be an identifier");
    }
    const binding = scope.get(target.text) ?? this.stateBindings.get(target.text);
    if (!binding) this.fail(target, `Unknown realtime variable ${target.text}`);
    return binding;
  }

  allocateLocal(name: string, scope: Scope): Binding {
    if (scope.bindings.has(name)) this.fail(this.sourceFile, `Duplicate realtime local ${name}`);
    if (this.#nextLocal >= 256) this.fail(this.sourceFile, "Realtime local limit (256) exceeded");
    const binding: Binding = { kind: "local", slot: this.#nextLocal++ };
    this.#maxLocal = Math.max(this.#maxLocal, this.#nextLocal);
    scope.bindings.set(name, binding);
    return binding;
  }

  requireValue(expression: ts.Expression, scope: Scope): void {
    if (!this.compileExpression(expression, scope)) {
      this.fail(expression, "Expression does not produce a realtime value");
    }
  }

  requireConstantNumber(expression: ts.Expression | undefined, owner: ts.Node): number {
    if (!expression) this.fail(owner, "Missing realtime intrinsic argument");
    const value = this.constantNumber(expression);
    if (value === undefined) this.fail(expression, "Argument must be a compile-time integer");
    return value;
  }

  requireConstantU16(expression: ts.Expression | undefined, owner: ts.Node): number {
    const value = this.requireConstantNumber(expression, owner);
    if (value < 0 || value > 0xffff) this.fail(expression ?? owner, "Argument must fit u16");
    return value;
  }

  requireConstantI32(expression: ts.Expression | undefined, owner: ts.Node): number {
    const value = this.requireConstantNumber(expression, owner);
    if (value < -0x8000_0000 || value > 0x7fff_ffff) {
      this.fail(expression ?? owner, "Argument must fit i32");
    }
    return value;
  }

  constantNumber(expression: ts.Expression): number | undefined {
    const value = this.constantValue(expression, new Set());
    if (value === undefined) return undefined;
    const number = Number(value);
    return Number.isSafeInteger(number) ? number : undefined;
  }

  constantString(expression: ts.Expression, seen = new Set<string>()): string | undefined {
    if (
      ts.isParenthesizedExpression(expression) ||
      ts.isAsExpression(expression) ||
      ts.isTypeAssertionExpression(expression) ||
      ts.isNonNullExpression(expression)
    ) {
      return this.constantString(expression.expression, seen);
    }
    if (ts.isStringLiteral(expression) || ts.isNoSubstitutionTemplateLiteral(expression)) {
      return expression.text;
    }
    if (!ts.isIdentifier(expression) || seen.has(expression.text)) return undefined;
    const initializer = this.constants.get(expression.text);
    if (!initializer) return undefined;
    seen.add(expression.text);
    const value = this.constantString(initializer, seen);
    seen.delete(expression.text);
    return value;
  }

  constantValue(expression: ts.Expression, seen: Set<string>): bigint | undefined {
    if (
      ts.isParenthesizedExpression(expression) ||
      ts.isAsExpression(expression) ||
      ts.isTypeAssertionExpression(expression) ||
      ts.isNonNullExpression(expression)
    ) {
      return this.constantValue(expression.expression, seen);
    }
    if (ts.isNumericLiteral(expression)) {
      const value = Number(expression.text);
      return Number.isSafeInteger(value) ? BigInt(value) : undefined;
    }
    if (expression.kind === ts.SyntaxKind.TrueKeyword) return 1n;
    if (expression.kind === ts.SyntaxKind.FalseKeyword) return 0n;
    if (ts.isIdentifier(expression)) {
      if (seen.has(expression.text)) return undefined;
      const initializer = this.constants.get(expression.text);
      if (!initializer) return undefined;
      seen.add(expression.text);
      const value = this.constantValue(initializer, seen);
      seen.delete(expression.text);
      return value;
    }
    if (ts.isPropertyAccessExpression(expression) && ts.isIdentifier(expression.expression)) {
      const owner = expression.expression.text;
      const member = expression.name.text;
      const value = enumMember(owner, member);
      return value === undefined ? undefined : BigInt(value);
    }
    if (ts.isPrefixUnaryExpression(expression)) {
      const value = this.constantValue(expression.operand, seen);
      if (value === undefined) return undefined;
      switch (expression.operator) {
        case ts.SyntaxKind.PlusToken:
          return value;
        case ts.SyntaxKind.MinusToken:
          return -value;
        case ts.SyntaxKind.TildeToken:
          return ~value;
        case ts.SyntaxKind.ExclamationToken:
          return value === 0n ? 1n : 0n;
        default:
          return undefined;
      }
    }
    if (ts.isBinaryExpression(expression)) {
      const left = this.constantValue(expression.left, seen);
      const right = this.constantValue(expression.right, seen);
      if (left === undefined || right === undefined) return undefined;
      switch (expression.operatorToken.kind) {
        case ts.SyntaxKind.PlusToken:
          return left + right;
        case ts.SyntaxKind.MinusToken:
          return left - right;
        case ts.SyntaxKind.AsteriskToken:
          return left * right;
        case ts.SyntaxKind.SlashToken:
          return right === 0n ? undefined : left / right;
        case ts.SyntaxKind.PercentToken:
          return right === 0n ? undefined : left % right;
        case ts.SyntaxKind.LessThanLessThanToken:
          return left << BigInt(Number(right & 63n));
        case ts.SyntaxKind.GreaterThanGreaterThanToken:
          return left >> BigInt(Number(right & 63n));
        case ts.SyntaxKind.AmpersandToken:
          return left & right;
        case ts.SyntaxKind.BarToken:
          return left | right;
        case ts.SyntaxKind.CaretToken:
          return left ^ right;
        default:
          return undefined;
      }
    }
    return undefined;
  }

  emit(opcode: Opcode, fields: Partial<Omit<Instruction, "opcode">> = {}): number {
    const op = instruction(opcode);
    Object.assign(op, fields);
    const index = this.code.length;
    this.code.push(op);
    return index;
  }

  patch(index: number, target: number): void {
    const op = this.code[index];
    if (!op) throw new Error(`internal compiler error: missing instruction ${index}`);
    op.b = target;
  }

  patchAll(indices: readonly number[], target: number): void {
    for (const index of indices) this.patch(index, target);
  }

  fail(node: ts.Node, message: string): never {
    const start = node.getStart(this.sourceFile, false);
    const position = this.sourceFile.getLineAndCharacterOfPosition(start);
    throw new SpellwireCompileError([
      {
        fileName: this.sourceFile.fileName,
        line: position.line + 1,
        column: position.character + 1,
        message,
      },
    ]);
  }

  rejectSyntaxErrors(): void {
    const diagnostics = (this.sourceFile as SourceFileWithParseDiagnostics).parseDiagnostics ?? [];
    if (diagnostics.length === 0) return;
    throw new SpellwireCompileError(
      diagnostics.map((diagnostic) => {
        const start = diagnostic.start ?? 0;
        const position = this.sourceFile.getLineAndCharacterOfPosition(start);
        return {
          fileName: this.sourceFile.fileName,
          line: position.line + 1,
          column: position.character + 1,
          message: ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n"),
        };
      }),
    );
  }
}

export function compileSource(source: string, options: CompileOptions = {}): CompileResult {
  const fileName = options.fileName ?? "macro.spellwire.ts";
  const stackLimit = options.stackLimit ?? 128;
  const instructionBudget = options.instructionBudget ?? 100_000;
  if (!Number.isSafeInteger(stackLimit) || stackLimit < 1 || stackLimit > 256) {
    throw optionError(fileName, "stackLimit must be an integer between 1 and 256");
  }
  if (
    !Number.isSafeInteger(instructionBudget) ||
    instructionBudget < 1 ||
    instructionBudget > 0xffff_ffff
  ) {
    throw optionError(fileName, "instructionBudget must be an integer between 1 and 4294967295");
  }
  const compiler = new Compiler(source, {
    fileName,
    stackLimit,
    instructionBudget,
  });
  return compiler.compile();
}

function optionError(fileName: string, message: string): SpellwireCompileError {
  return new SpellwireCompileError([{ fileName, line: 1, column: 1, message }]);
}

function binaryOpcode(kind: ts.SyntaxKind): Opcode | undefined {
  switch (kind) {
    case ts.SyntaxKind.PlusToken:
      return Opcode.Add;
    case ts.SyntaxKind.MinusToken:
      return Opcode.Sub;
    case ts.SyntaxKind.AsteriskToken:
      return Opcode.Mul;
    case ts.SyntaxKind.SlashToken:
      return Opcode.Div;
    case ts.SyntaxKind.PercentToken:
      return Opcode.Mod;
    case ts.SyntaxKind.EqualsEqualsToken:
    case ts.SyntaxKind.EqualsEqualsEqualsToken:
      return Opcode.Eq;
    case ts.SyntaxKind.ExclamationEqualsToken:
    case ts.SyntaxKind.ExclamationEqualsEqualsToken:
      return Opcode.Ne;
    case ts.SyntaxKind.LessThanToken:
      return Opcode.Lt;
    case ts.SyntaxKind.LessThanEqualsToken:
      return Opcode.Le;
    case ts.SyntaxKind.GreaterThanToken:
      return Opcode.Gt;
    case ts.SyntaxKind.GreaterThanEqualsToken:
      return Opcode.Ge;
    case ts.SyntaxKind.AmpersandToken:
      return Opcode.BitAnd;
    case ts.SyntaxKind.BarToken:
      return Opcode.BitOr;
    case ts.SyntaxKind.CaretToken:
      return Opcode.BitXor;
    case ts.SyntaxKind.LessThanLessThanToken:
      return Opcode.Shl;
    case ts.SyntaxKind.GreaterThanGreaterThanToken:
      return Opcode.Shr;
    default:
      return undefined;
  }
}

function compoundOpcode(kind: ts.SyntaxKind): Opcode | undefined {
  switch (kind) {
    case ts.SyntaxKind.PlusEqualsToken:
      return Opcode.Add;
    case ts.SyntaxKind.MinusEqualsToken:
      return Opcode.Sub;
    case ts.SyntaxKind.AsteriskEqualsToken:
      return Opcode.Mul;
    case ts.SyntaxKind.SlashEqualsToken:
      return Opcode.Div;
    case ts.SyntaxKind.PercentEqualsToken:
      return Opcode.Mod;
    case ts.SyntaxKind.AmpersandEqualsToken:
      return Opcode.BitAnd;
    case ts.SyntaxKind.BarEqualsToken:
      return Opcode.BitOr;
    case ts.SyntaxKind.CaretEqualsToken:
      return Opcode.BitXor;
    case ts.SyntaxKind.LessThanLessThanEqualsToken:
      return Opcode.Shl;
    case ts.SyntaxKind.GreaterThanGreaterThanEqualsToken:
      return Opcode.Shr;
    default:
      return undefined;
  }
}

function callName(expression: ts.LeftHandSideExpression): string | undefined {
  if (ts.isIdentifier(expression)) return expression.text;
  return undefined;
}

function enumMember(owner: string, member: string): number | undefined {
  const source = owner === "Key"
    ? Key
    : owner === "MouseButton"
      ? MouseButton
      : owner === "InputSource"
        ? InputSource
        : owner === "Modifier"
          ? Modifier
        : undefined;
  if (!source) return undefined;
  const value = source[member as keyof typeof source];
  return typeof value === "number" ? value : undefined;
}

function packPair(x: number, y: number): bigint {
  const low = BigInt(x >>> 0);
  const high = BigInt(y >>> 0) << 32n;
  return BigInt.asIntN(64, high | low);
}

function formatDiagnostic(diagnostic: CompileDiagnostic): string {
  return `${diagnostic.fileName}:${diagnostic.line}:${diagnostic.column}: ${diagnostic.message}`;
}
