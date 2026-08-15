// Public domain view of a scene snapshot, mapped from the proto SceneSnapshot
// pushed on the /events channel. User code (and the React read model) consumes
// these types, never the generated proto.
//
// Each stimulus carries both its stable UUID `id` and its u32 server `handle`
// (the map key used to address mutations like SetPosition during RF mapping).

import type { SceneSnapshot as ProtoSnapshot } from "./_proto/vstimd/v1/snapshot_pb.js";
import type { QueryStimulusResponse } from "./_proto/vstimd/v1/stimuli/query_pb.js";
import { StimulusType } from "./_proto/vstimd/v1/stimuli/stimulus_type_pb.js";
import {
  VirtualTriggerLineKind,
  type VirtualTriggerLineInfo,
} from "./_proto/vstimd/v1/vtl_pb.js";
import { toServerInfo, type ServerInfo } from "./system.js";
import type { VtlKind } from "./vtl.js";
import type { Color, StimulusHandle, StimulusKind, Vec2 } from "./types.js";

export interface StimulusView {
  /** Stable UUID assigned at creation. */
  id: string;
  /** Human-readable label ("" if unset). */
  name: string;
  /** Server handle (map key) — addresses mutations like setPosition. */
  handle: StimulusHandle;
  kind: StimulusKind;
  pos: Vec2;
  /** Bounding-box size in stimulus-space pixels (full width/height). */
  size: { width: number; height: number };
  /** Orientation in degrees CCW. */
  orientation: number;
  opacity: number;
  fillColor?: Color;
  enabled: boolean;
  drawOrder: number;
}

export interface VtlLineView {
  name: string;
  bank: number;
  bit: number;
  kind: VtlKind;
  /** Current level (true = high). */
  high: boolean;
}

/** Map a proto VTL line onto the public view. Shared with `conn.vtl.list()`. */
export function toVtlLineView(l: VirtualTriggerLineInfo): VtlLineView {
  return {
    name: l.name,
    bank: l.bank,
    bit: l.bit,
    kind:
      l.kind === VirtualTriggerLineKind.OUTPUT ? "output" : "input",
    high: l.high,
  };
}

export interface SceneSnapshot {
  serverInfo?: ServerInfo;
  stimuli: StimulusView[];
  vtlLines: VtlLineView[];
  frameCount: bigint;
  serverTimeNs: bigint;
}

function kindOf(t: StimulusType): StimulusKind {
  switch (t) {
    case StimulusType.RECT: return "rect";
    case StimulusType.CIRCLE: return "circle";
    case StimulusType.ELLIPSE: return "ellipse";
    case StimulusType.GRATING: return "grating";
    case StimulusType.TEXT: return "text";
    case StimulusType.POLYGON: return "polygon";
    default: return "unknown";
  }
}

/** The colour to draw a stimulus with on the map — per type, since only shapes
 * have a fill. Gratings show their peak colour, text its glyph colour. */
function fillColorOf(s: QueryStimulusResponse): Color | undefined {
  const shape = s.params?.shape;
  switch (shape?.case) {
    case "rect":
    case "ellipse":
    case "circle":
    case "polygon":
      return shape.value.appearance?.fillColor;
    case "grating":
      return shape.value.foreColor;
    case "text":
      return shape.value.textColor;
    default:
      return undefined;
  }
}

/** Bounding-box size in stimulus-space pixels from the shape params. */
function sizeOf(s: QueryStimulusResponse): { width: number; height: number } {
  const shape = s.params?.shape;
  switch (shape?.case) {
    case "rect":
    case "ellipse":
    case "grating":
      return { width: shape.value.width, height: shape.value.height };
    case "circle":
      return { width: shape.value.radius * 2, height: shape.value.radius * 2 };
    case "text":
      return {
        width: shape.value.boxSize?.x ?? 0,
        height: shape.value.boxSize?.y ?? 0,
      };
    default:
      return { width: 20, height: 20 };
  }
}

export function toSceneSnapshot(p: ProtoSnapshot): SceneSnapshot {
  return {
    serverInfo: p.serverInfo ? toServerInfo(p.serverInfo) : undefined,
    stimuli: p.stimuli.map((s) => ({
      id: s.id,
      name: s.name,
      handle: s.handle,
      kind: kindOf(s.stimulusType),
      // Placement is a oneof, per dimension. Only 2-D stimuli exist today; a
      // 3-D one would report a transform this map cannot draw, so it falls back
      // to the origin rather than inventing coordinates.
      pos: {
        x: s.placement.case === "transform2d" ? (s.placement.value.pos?.x ?? 0) : 0,
        y: s.placement.case === "transform2d" ? (s.placement.value.pos?.y ?? 0) : 0,
      },
      size: sizeOf(s),
      orientation: s.placement.case === "transform2d" ? s.placement.value.rotationDeg : 0,
      opacity: s.opacity,
      fillColor: fillColorOf(s),
      enabled: s.enabled,
      drawOrder: s.drawOrder,
    })),
    vtlLines: (p.vtlLines?.lines ?? []).map(toVtlLineView),
    frameCount: p.frameCount,
    serverTimeNs: p.serverTimeNs,
  };
}
