import {
  NativeOverlayRenderer,
  OverlayScene,
  type NativeOverlayOptions,
  type OverlayNode,
  type OverlayNodeId,
  type OverlayShadow,
  type OverlayStroke,
} from "./overlay";

export type OverlayLength = number | "fill";
export type OverlayAlign = "start" | "center" | "end" | "stretch";
export type OverlayJustify = "start" | "center" | "end" | "space-between";

export interface OverlayInsets {
  readonly x?: number;
  readonly y?: number;
  readonly top?: number;
  readonly right?: number;
  readonly bottom?: number;
  readonly left?: number;
}

export interface OverlayLayoutProps {
  readonly key?: string;
  readonly x?: number;
  readonly y?: number;
  readonly width?: OverlayLength;
  readonly height?: OverlayLength;
  readonly minWidth?: number;
  readonly minHeight?: number;
  readonly maxWidth?: number;
  readonly maxHeight?: number;
  readonly opacity?: number;
}

export interface OverlayFrameProps extends OverlayLayoutProps {
  readonly padding?: number | OverlayInsets;
  readonly gap?: number;
  readonly align?: OverlayAlign;
  readonly justify?: OverlayJustify;
  readonly fill?: string;
  readonly radius?: number;
  readonly stroke?: string | OverlayStroke;
  readonly shadow?: OverlayShadow;
}

export interface OverlayTextProps extends OverlayLayoutProps {
  readonly fill?: string;
  readonly fontFamily?: "system" | "monospace";
  readonly fontSize?: number;
  readonly fontWeight?: number;
  readonly lineHeight?: number;
  readonly letterSpacing?: number;
  readonly textAlign?: "left" | "center" | "right";
}

export interface OverlayEllipseProps extends OverlayLayoutProps {
  readonly fill?: string;
  readonly stroke?: string | OverlayStroke;
  readonly shadow?: OverlayShadow;
}

export interface OverlayDividerProps {
  readonly key?: string;
  readonly fill?: string;
  readonly width?: OverlayLength;
  readonly height?: number;
  readonly opacity?: number;
}

export interface OverlayDotProps extends Omit<OverlayEllipseProps, "width" | "height"> {
  readonly size?: number;
}

export interface OverlayBadgeProps extends OverlayFrameProps {
  readonly textFill?: string;
  readonly fontFamily?: "system" | "monospace";
  readonly fontSize?: number;
  readonly fontWeight?: number;
}

type FrameDirection = "row" | "column" | "stack";

interface FrameElement {
  readonly kind: "frame";
  readonly direction: FrameDirection;
  readonly props: OverlayFrameProps;
  readonly children: readonly OverlayElement[];
}

interface TextElement {
  readonly kind: "text";
  readonly text: string;
  readonly props: OverlayTextProps;
}

interface EllipseElement {
  readonly kind: "ellipse";
  readonly props: OverlayEllipseProps;
}

interface SpacerElement {
  readonly kind: "spacer";
  readonly props: OverlayLayoutProps;
}

interface BindingElement {
  readonly kind: "binding";
  readonly key?: string;
  readonly source: object | (() => unknown);
  readonly read: () => unknown;
  readonly render: (value: unknown) => OverlayChild;
  readonly equals: (left: unknown, right: unknown) => boolean;
}

export type OverlayElement =
  | FrameElement
  | TextElement
  | EllipseElement
  | SpacerElement
  | BindingElement;

export type OverlayChild =
  | OverlayElement
  | readonly OverlayChild[]
  | false
  | null
  | undefined;

export interface OverlayReadable<T> {
  get(): T;
}

export interface OverlayStateSource<T> {
  snapshotStates(): T;
}

export interface OverlayBindingOptions<T> {
  readonly key?: string;
  readonly equals?: (left: T, right: T) => boolean;
}

export interface OverlayMountOptions extends NativeOverlayOptions {
  /** State-source polls per second. Static trees create no timer. Default: 30. */
  readonly fps?: number;
  /** Reuse an already-started renderer, mainly for multi-view control and tests. */
  readonly renderer?: NativeOverlayRenderer;
  readonly onError?: (error: Error) => void;
}

function frameElement(
  direction: FrameDirection,
  props: OverlayFrameProps,
  children: readonly OverlayChild[],
): FrameElement {
  return { kind: "frame", direction, props, children: flattenChildren(children) };
}

export const ui = {
  box(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("stack", props, children);
  },

  frame(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("stack", props, children);
  },

  stack(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("stack", props, children);
  },

  row(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("row", props, children);
  },

  column(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("column", props, children);
  },

  panel(props: OverlayFrameProps = {}, ...children: OverlayChild[]): OverlayElement {
    return frameElement("column", props, children);
  },

  text(text: string | number, props: OverlayTextProps = {}): OverlayElement {
    return { kind: "text", text: String(text), props };
  },

  spacer(size: number | OverlayLayoutProps = 0): OverlayElement {
    return {
      kind: "spacer",
      props: typeof size === "number" ? { width: size, height: size } : size,
    };
  },

  divider(props: OverlayDividerProps = {}): OverlayElement {
    return frameElement(
      "stack",
      {
        ...(props.key === undefined ? {} : { key: props.key }),
        width: props.width ?? "fill",
        height: props.height ?? 1,
        fill: props.fill ?? "#ffffff24",
        ...(props.opacity === undefined ? {} : { opacity: props.opacity }),
      },
      [],
    );
  },

  dot(props: OverlayDotProps = {}): OverlayElement {
    const size = props.size ?? 8;
    return {
      kind: "ellipse",
      props: {
        ...props,
        width: size,
        height: size,
      },
    };
  },

  ellipse(props: OverlayEllipseProps = {}): OverlayElement {
    return { kind: "ellipse", props };
  },

  badge(label: string | number, props: OverlayBadgeProps = {}): OverlayElement {
    const {
      textFill = "#ffffffff",
      fontFamily = "system",
      fontSize = 12,
      fontWeight = 600,
      ...frameProps
    } = props;
    return frameElement(
      "row",
      {
        padding: { x: 8, y: 4 },
        radius: 999,
        fill: "#ffffff18",
        align: "center",
        ...frameProps,
      },
      [ui.text(label, { fill: textFill, fontFamily, fontSize, fontWeight })],
    );
  },

  bind<T>(
    source: OverlayReadable<T> | OverlayStateSource<T> | (() => T),
    render: (value: T) => OverlayChild,
    options: OverlayBindingOptions<T> = {},
  ): OverlayElement {
    return createBinding(source, render, options);
  },

  when(
    source: OverlayReadable<boolean> | (() => boolean),
    content: OverlayChild | (() => OverlayChild),
    fallback: OverlayChild = null,
  ): OverlayElement {
    return createBinding(
      source,
      (visible) =>
        visible ? (typeof content === "function" ? content() : content) : fallback,
      {},
    );
  },
} as const;

function createBinding<T>(
  source: OverlayReadable<T> | OverlayStateSource<T> | (() => T),
  render: (value: T) => OverlayChild,
  options: OverlayBindingOptions<T>,
): BindingElement {
  const read = (): T => {
    if (typeof source === "function") return source();
    if ("snapshotStates" in source) return source.snapshotStates();
    return source.get();
  };
  return {
    kind: "binding",
    ...(options.key === undefined ? {} : { key: options.key }),
    source,
    read,
    render: (value) => render(value as T),
    equals: (left, right) =>
      options.equals?.(left as T, right as T) ?? shallowValueEqual(left, right),
  };
}

function flattenChildren(children: readonly OverlayChild[]): OverlayElement[] {
  const output: OverlayElement[] = [];
  const append = (child: OverlayChild): void => {
    if (child === null || child === undefined || child === false) return;
    if (Array.isArray(child)) {
      for (const nested of child) append(nested);
    } else {
      output.push(child as OverlayElement);
    }
  };
  for (const child of children) append(child);
  return output;
}

interface BindingCache {
  value: unknown;
  rendered: OverlayElement | null;
  initialized: boolean;
}

interface Size {
  width: number;
  height: number;
}

interface Primitive {
  key: string;
  node: OverlayNode;
}

class OverlayCompiler {
  readonly #bindings = new Map<string, BindingCache>();
  #bindingCount = 0;
  #lastResolved: OverlayElement | null = null;

  get hasBindings(): boolean {
    return this.#bindingCount > 0;
  }

  compile(root: OverlayElement, force: boolean): readonly Primitive[] | null {
    const reads = new Map<object | (() => unknown), unknown>();
    const active = new Set<string>();
    const status = { changed: force, count: 0, force };
    const resolved = this.#resolve(root, "root", reads, active, status);
    this.#bindingCount = status.count;
    for (const key of this.#bindings.keys()) {
      if (!active.has(key)) this.#bindings.delete(key);
    }
    if (!status.changed && this.#lastResolved !== null) return null;
    this.#lastResolved = resolved;
    if (resolved === null) return [];
    const output: Primitive[] = [];
    const z = { value: 0 };
    layoutElement(resolved, 0, 0, undefined, undefined, 1, "root", output, z);
    return output;
  }

  #resolve(
    element: OverlayElement,
    path: string,
    reads: Map<object | (() => unknown), unknown>,
    active: Set<string>,
    status: { changed: boolean; count: number; force: boolean },
  ): OverlayElement | null {
    if (element.kind === "binding") {
      status.count += 1;
      const bindingKey = `${path}/$binding`;
      active.add(bindingKey);
      let value: unknown;
      if (reads.has(element.source)) {
        value = reads.get(element.source);
      } else {
        value = element.read();
        reads.set(element.source, value);
      }
      let cache = this.#bindings.get(bindingKey);
      if (!cache) {
        cache = { value: undefined, rendered: null, initialized: false };
        this.#bindings.set(bindingKey, cache);
      }
      if (status.force || !cache.initialized || !element.equals(cache.value, value)) {
        cache.value = cloneBindingValue(value);
        cache.rendered = singleElement(element.render(value));
        cache.initialized = true;
        status.changed = true;
      }
      return cache.rendered === null
        ? null
        : this.#resolve(cache.rendered, `${path}/value`, reads, active, status);
    }

    if (element.kind !== "frame") return element;
    const children: OverlayElement[] = [];
    for (let index = 0; index < element.children.length; index += 1) {
      const child = element.children[index];
      if (!child) continue;
      const key = elementKey(child, index);
      const resolved = this.#resolve(child, `${path}/${key}`, reads, active, status);
      if (resolved !== null) children.push(resolved);
    }
    return { ...element, children };
  }
}

function singleElement(child: OverlayChild): OverlayElement | null {
  const children = flattenChildren([child]);
  if (children.length === 0) return null;
  if (children.length === 1) return children[0] ?? null;
  return frameElement("column", {}, children);
}

function elementKey(element: OverlayElement, index: number): string {
  if (element.kind === "binding") return element.key ?? String(index);
  return element.props.key ?? String(index);
}

function naturalSize(element: OverlayElement): Size {
  if (element.kind === "binding") return { width: 0, height: 0 };
  if (element.kind === "text") {
    const props = element.props;
    const fontSize = props.fontSize ?? 14;
    const lineHeight = props.lineHeight ?? fontSize * 1.2;
    return boundedSize(
      props,
      numericLength(props.width) ?? estimateTextWidth(element.text, fontSize, props.letterSpacing),
      numericLength(props.height) ?? lineHeight,
    );
  }
  if (element.kind === "ellipse") {
    const props = element.props;
    return boundedSize(
      props,
      numericLength(props.width) ?? 8,
      numericLength(props.height) ?? 8,
    );
  }
  if (element.kind === "spacer") {
    const props = element.props;
    return boundedSize(
      props,
      numericLength(props.width) ?? 0,
      numericLength(props.height) ?? 0,
    );
  }

  const props = element.props;
  const padding = insets(props.padding);
  const gap = finite(props.gap, 0);
  const sizes = element.children.map(naturalSize);
  let contentWidth = 0;
  let contentHeight = 0;
  if (element.direction === "row") {
    contentWidth = sizes.reduce((sum, size) => sum + size.width, 0) + gapCount(sizes, gap);
    contentHeight = sizes.reduce((maximum, size) => Math.max(maximum, size.height), 0);
  } else if (element.direction === "column") {
    contentWidth = sizes.reduce((maximum, size) => Math.max(maximum, size.width), 0);
    contentHeight = sizes.reduce((sum, size) => sum + size.height, 0) + gapCount(sizes, gap);
  } else {
    for (let index = 0; index < element.children.length; index += 1) {
      const child = element.children[index];
      const size = sizes[index];
      if (!child || !size) continue;
      const childProps = child.kind === "binding" ? {} : child.props;
      contentWidth = Math.max(contentWidth, finite(childProps.x, 0) + size.width);
      contentHeight = Math.max(contentHeight, finite(childProps.y, 0) + size.height);
    }
  }
  return boundedSize(
    props,
    numericLength(props.width) ?? contentWidth + padding.left + padding.right,
    numericLength(props.height) ?? contentHeight + padding.top + padding.bottom,
  );
}

function layoutElement(
  element: OverlayElement,
  originX: number,
  originY: number,
  forcedWidth: number | undefined,
  forcedHeight: number | undefined,
  parentOpacity: number,
  path: string,
  output: Primitive[],
  z: { value: number },
): Size {
  if (element.kind === "binding") return { width: 0, height: 0 };
  const props = element.props;
  const natural = naturalSize(element);
  const width = boundedDimension(
    forcedWidth ?? numericLength(props.width) ?? natural.width,
    props.minWidth,
    props.maxWidth,
  );
  const height = boundedDimension(
    forcedHeight ?? numericLength(props.height) ?? natural.height,
    props.minHeight,
    props.maxHeight,
  );
  const x = originX + finite(props.x, 0);
  const y = originY + finite(props.y, 0);
  const opacity = parentOpacity * Math.max(0, Math.min(1, finite(props.opacity, 1)));

  if (element.kind === "text") {
    const textProps = element.props;
    output.push({
      key: `${path}/text`,
      node: {
        kind: "text",
        x,
        y,
        width,
        height,
        text: element.text,
        size: textProps.fontSize ?? 14,
        fill: textProps.fill ?? "#ffffffff",
        opacity,
        font: {
          family: textProps.fontFamily ?? "system",
          weight: textProps.fontWeight ?? 400,
          lineHeight: textProps.lineHeight ?? (textProps.fontSize ?? 14) * 1.2,
          letterSpacing: textProps.letterSpacing ?? 0,
          align: textProps.textAlign ?? "left",
        },
        z: z.value++,
      },
    });
    return { width, height };
  }

  if (element.kind === "ellipse") {
    const ellipseProps = element.props;
    output.push({
      key: `${path}/ellipse`,
      node: {
        kind: "ellipse",
        x,
        y,
        width,
        height,
        fill: ellipseProps.fill ?? "#ffffffff",
        ...(ellipseProps.stroke === undefined
          ? {}
          : { stroke: normalizeStroke(ellipseProps.stroke) }),
        ...(ellipseProps.shadow === undefined ? {} : { shadow: ellipseProps.shadow }),
        opacity,
        z: z.value++,
      },
    });
    return { width, height };
  }

  if (element.kind === "spacer") return { width, height };

  const frameProps = element.props;
  if (
    frameProps.fill !== undefined ||
    frameProps.stroke !== undefined ||
    frameProps.shadow !== undefined
  ) {
    output.push({
      key: `${path}/frame`,
      node: {
        kind: "rect",
        x,
        y,
        width,
        height,
        radius: finite(frameProps.radius, 0),
        fill: frameProps.fill ?? "#00000000",
        ...(frameProps.stroke === undefined
          ? {}
          : { stroke: normalizeStroke(frameProps.stroke) }),
        ...(frameProps.shadow === undefined ? {} : { shadow: frameProps.shadow }),
        opacity,
        z: z.value++,
      },
    });
  }

  const padding = insets(frameProps.padding);
  const contentX = x + padding.left;
  const contentY = y + padding.top;
  const contentWidth = Math.max(0, width - padding.left - padding.right);
  const contentHeight = Math.max(0, height - padding.top - padding.bottom);
  if (element.direction === "stack") {
    for (let index = 0; index < element.children.length; index += 1) {
      const child = element.children[index];
      if (!child) continue;
      const childProps = child.kind === "binding" ? {} : child.props;
      const childSize = naturalSize(child);
      const childWidth = childProps.width === "fill" ? contentWidth : undefined;
      const childHeight = childProps.height === "fill" ? contentHeight : undefined;
      layoutElement(
        child,
        contentX,
        contentY,
        childWidth,
        childHeight,
        opacity,
        `${path}/${elementKey(child, index)}`,
        output,
        z,
      );
      void childSize;
    }
    return { width, height };
  }

  layoutFlowChildren(
    element,
    contentX,
    contentY,
    contentWidth,
    contentHeight,
    opacity,
    path,
    output,
    z,
  );
  return { width, height };
}

function layoutFlowChildren(
  element: FrameElement,
  x: number,
  y: number,
  width: number,
  height: number,
  opacity: number,
  path: string,
  output: Primitive[],
  z: { value: number },
): void {
  const row = element.direction === "row";
  const sizes = element.children.map(naturalSize);
  const mainAvailable = row ? width : height;
  const crossAvailable = row ? height : width;
  const baseGap = finite(element.props.gap, 0);
  const gapTotal = gapCount(sizes, baseGap);
  const fillIndexes: number[] = [];
  let fixed = 0;
  for (let index = 0; index < element.children.length; index += 1) {
    const child = element.children[index];
    const size = sizes[index];
    if (!child || !size) continue;
    const props = child.kind === "binding" ? {} : child.props;
    const fill = row ? props.width === "fill" : props.height === "fill";
    if (fill) fillIndexes.push(index);
    else fixed += row ? size.width : size.height;
  }
  const fillSize = fillIndexes.length === 0
    ? 0
    : Math.max(0, mainAvailable - fixed - gapTotal) / fillIndexes.length;
  const mainSizes = sizes.map((size, index) =>
    fillIndexes.includes(index) ? fillSize : row ? size.width : size.height,
  );
  const used = mainSizes.reduce((sum, value) => sum + value, 0) + gapTotal;
  let gap = baseGap;
  let cursor = 0;
  switch (element.props.justify) {
    case "center":
      cursor = (mainAvailable - used) / 2;
      break;
    case "end":
      cursor = mainAvailable - used;
      break;
    case "space-between":
      gap = sizes.length > 1
        ? Math.max(baseGap, (mainAvailable - mainSizes.reduce((sum, value) => sum + value, 0)) / (sizes.length - 1))
        : 0;
      break;
    default:
      break;
  }

  for (let index = 0; index < element.children.length; index += 1) {
    const child = element.children[index];
    const size = sizes[index];
    const mainSize = mainSizes[index];
    if (!child || !size || mainSize === undefined) continue;
    const childProps = child.kind === "binding" ? {} : child.props;
    const naturalCross = row ? size.height : size.width;
    const fillCross = row ? childProps.height === "fill" : childProps.width === "fill";
    const align = element.props.align ?? "start";
    const crossSize = fillCross || align === "stretch" ? crossAvailable : naturalCross;
    let cross = 0;
    if (align === "center") cross = (crossAvailable - crossSize) / 2;
    if (align === "end") cross = crossAvailable - crossSize;
    layoutElement(
      child,
      row ? x + cursor : x + cross,
      row ? y + cross : y + cursor,
      row ? mainSize : crossSize,
      row ? crossSize : mainSize,
      opacity,
      `${path}/${elementKey(child, index)}`,
      output,
      z,
    );
    cursor += mainSize + gap;
  }
}

export class OverlayView {
  readonly scene = new OverlayScene();
  readonly #compiler = new OverlayCompiler();
  readonly #ids = new Map<string, OverlayNodeId>();
  #root: OverlayElement;

  constructor(root: OverlayElement) {
    this.#root = root;
  }

  get hasBindings(): boolean {
    return this.#compiler.hasBindings;
  }

  set(root: OverlayElement): number {
    this.#root = root;
    return this.#render(true);
  }

  refresh(): number {
    return this.#render(false);
  }

  #render(force: boolean): number {
    const primitives = this.#compiler.compile(this.#root, force);
    if (primitives === null) return 0;
    const active = new Set<string>();
    let mutations = 0;
    for (const primitive of primitives) {
      active.add(primitive.key);
      const id = this.#ids.get(primitive.key);
      if (id === undefined) {
        this.#ids.set(primitive.key, this.scene.create(primitive.node));
        mutations += 1;
      } else if (this.scene.update(id, primitive.node)) {
        mutations += 1;
      }
    }
    for (const [key, id] of this.#ids) {
      if (active.has(key)) continue;
      this.scene.remove(id);
      this.#ids.delete(key);
      mutations += 1;
    }
    return mutations;
  }
}

/** High-level retained overlay: declarative layout, state bindings, keyed diff, native renderer. */
export class Overlay {
  readonly renderer: NativeOverlayRenderer;
  readonly view: OverlayView;
  readonly fps: number;

  #timer: ReturnType<typeof setInterval> | undefined;
  #tail: Promise<number> = Promise.resolve(0);
  #timerRefreshInFlight = false;
  #closed = false;
  readonly #onError: ((error: Error) => void) | undefined;

  private constructor(
    renderer: NativeOverlayRenderer,
    view: OverlayView,
    fps: number,
    onError: ((error: Error) => void) | undefined,
  ) {
    this.renderer = renderer;
    this.view = view;
    this.fps = fps;
    this.#onError = onError;
  }

  static async mount(root: OverlayElement, options: OverlayMountOptions = {}): Promise<Overlay> {
    const fps = options.fps ?? 30;
    if (!Number.isFinite(fps) || fps < 0 || fps > 240) {
      throw new RangeError("overlay fps must be between 0 and 240");
    }
    const renderer = options.renderer ?? await NativeOverlayRenderer.start({
      ...(options.executablePath === undefined ? {} : { executablePath: options.executablePath }),
      ...(options.readyTimeoutMs === undefined ? {} : { readyTimeoutMs: options.readyTimeoutMs }),
    });
    const view = new OverlayView(root);
    const overlay = new Overlay(renderer, view, fps, options.onError);
    try {
      view.set(root);
      await renderer.apply(view.scene);
      overlay.#syncTimer();
      return overlay;
    } catch (error) {
      if (options.renderer === undefined) await renderer.close().catch(() => undefined);
      throw error;
    }
  }

  set(root: OverlayElement): Promise<number> {
    this.#assertOpen();
    return this.#enqueue(async () => {
      this.view.set(root);
      const applied = await this.renderer.apply(this.view.scene);
      this.#syncTimer();
      return applied;
    });
  }

  refresh(): Promise<number> {
    this.#assertOpen();
    return this.#enqueue(async () => {
      this.view.refresh();
      return this.renderer.apply(this.view.scene);
    });
  }

  async show(): Promise<void> {
    this.#assertOpen();
    await this.renderer.show();
  }

  async hide(): Promise<void> {
    this.#assertOpen();
    await this.renderer.hide();
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    if (this.#timer !== undefined) clearInterval(this.#timer);
    await this.#tail.catch(() => 0);
    await this.renderer.close();
  }

  #enqueue(task: () => Promise<number>): Promise<number> {
    const next = this.#tail.then(task);
    this.#tail = next.catch(() => 0);
    return next;
  }

  #syncTimer(): void {
    const shouldRun = this.view.hasBindings && this.fps > 0;
    if (shouldRun && this.#timer === undefined) {
      this.#timer = setInterval(() => {
        if (this.#closed || this.#timerRefreshInFlight) return;
        this.#timerRefreshInFlight = true;
        void this.refresh()
          .catch((error: unknown) => {
            const failure = error instanceof Error ? error : new Error(String(error));
            if (this.#onError) this.#onError(failure);
            else console.error(`Spellwire overlay refresh failed: ${failure.message}`);
          })
          .finally(() => {
            this.#timerRefreshInFlight = false;
          });
      }, 1_000 / this.fps);
    } else if (!shouldRun && this.#timer !== undefined) {
      clearInterval(this.#timer);
      this.#timer = undefined;
    }
  }

  #assertOpen(): void {
    if (this.#closed) throw new Error("Spellwire overlay is closed");
  }
}

function normalizeStroke(stroke: string | OverlayStroke): OverlayStroke {
  return typeof stroke === "string" ? { fill: stroke, width: 1 } : stroke;
}

function numericLength(value: OverlayLength | undefined): number | undefined {
  return typeof value === "number" ? Math.max(0, finite(value, 0)) : undefined;
}

function boundedSize(props: OverlayLayoutProps, width: number, height: number): Size {
  return {
    width: boundedDimension(width, props.minWidth, props.maxWidth),
    height: boundedDimension(height, props.minHeight, props.maxHeight),
  };
}

function boundedDimension(value: number, minimum?: number, maximum?: number): number {
  const min = Math.max(0, finite(minimum, 0));
  const max = maximum === undefined ? Number.POSITIVE_INFINITY : Math.max(min, finite(maximum, min));
  return Math.max(min, Math.min(max, Math.max(0, finite(value, 0))));
}

function finite(value: number | undefined, fallback: number): number {
  return value !== undefined && Number.isFinite(value) ? value : fallback;
}

function insets(value: number | OverlayInsets | undefined): Required<OverlayInsets> {
  if (typeof value === "number") {
    const size = Math.max(0, finite(value, 0));
    return { x: size, y: size, top: size, right: size, bottom: size, left: size };
  }
  const x = Math.max(0, finite(value?.x, 0));
  const y = Math.max(0, finite(value?.y, 0));
  return {
    x,
    y,
    top: Math.max(0, finite(value?.top, y)),
    right: Math.max(0, finite(value?.right, x)),
    bottom: Math.max(0, finite(value?.bottom, y)),
    left: Math.max(0, finite(value?.left, x)),
  };
}

function gapCount(sizes: readonly Size[], gap: number): number {
  return Math.max(0, sizes.length - 1) * Math.max(0, gap);
}

function estimateTextWidth(text: string, fontSize: number, letterSpacing = 0): number {
  let units = 0;
  let count = 0;
  for (const character of text) {
    count += 1;
    const code = character.codePointAt(0)!;
    if (character === " ") units += 0.33;
    else if (code > 0xff) units += 1;
    else if (character === "W" || character === "M" || character === "@" || character === "%") {
      units += 0.9;
    } else if (
      character === "i" || character === "l" || character === "I" || character === "!" ||
      character === "." || character === "," || character === ":" || character === ";"
    ) {
      units += 0.3;
    } else if (code >= 0x41 && code <= 0x5a) units += 0.68;
    else if (code >= 0x30 && code <= 0x39) units += 0.58;
    else units += 0.55;
  }
  return units * fontSize + Math.max(0, count - 1) * letterSpacing;
}

function shallowValueEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (!isRecord(left) || !isRecord(right)) return false;
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);
  return leftKeys.length === rightKeys.length && leftKeys.every(
    (key) => Object.hasOwn(right, key) && Object.is(left[key], right[key]),
  );
}

function cloneBindingValue(value: unknown): unknown {
  return isRecord(value) && !Object.isFrozen(value) ? { ...value } : value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
