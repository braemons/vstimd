// Hand-written public domain types. These are the only types user code (and the
// React app) touches — the generated protobuf-es classes under src/_proto stay
// private, exactly as the Python client hides its _proto package.

/** A 2-D point in stimulus space: origin at screen centre, +x right, +y up, pixels. */
export interface Vec2 {
  x: number;
  y: number;
}

/** RGBA colour, each channel in [0, 1]. */
export interface Color {
  r: number;
  g: number;
  b: number;
  a: number;
}

export function rgb(r: number, g: number, b: number, a = 1): Color {
  return { r, g, b, a };
}

/** Opaque handle to a stimulus on the server. */
export type StimulusHandle = number;

/** Opaque handle to an animation on the server. */
export type AnimationHandle = number;

/** Stimulus type, mirroring the wire's `StimulusType` enum as a string union.
 *
 * Not "kind": `StimulusKind` is the server's *internal* taxonomy, one arm per
 * render pipeline (Shape, Grating, Text, Mesh3d), where rect/circle/ellipse/polygon
 * are all one `Shape`. That name must not reach a client — this is the finer,
 * user-facing set. */
export type StimulusType =
  | "rect"
  | "circle"
  | "ellipse"
  | "grating"
  | "text"
  | "polygon"
  | "unknown";
