// The rolling trace behind the input-device plot: what to keep, what to derive,
// and how to scale it. Pure functions, so the panel is only a view and this is
// testable without a DOM.
//
// **This is a monitor, not a record.** Points come from the `/events` snapshot
// stream at ~30 Hz, which is a decimated view of a device sampled far faster —
// vstimd itself reads the segment every frame, and the producer writes it
// faster still. Speed here is therefore the slope between two snapshots, good
// enough to see an animal start and stop and to catch a reader that died, and
// not the trace anybody should analyse. The per-sample path of record belongs
// to the daemon that owns the device.

import type { InputDeviceView } from "../index.js";

/** One snapshot's reading of one axis. `t` is server time in seconds. */
export interface TracePoint {
  t: number;
  /** The axis' scaled value: a position for absolute and cumulative axes. */
  value: number;
  /** Units per second: derived for absolute/cumulative axes, read for rate ones. */
  speed: number;
  /** The producer was silent when this was read — so a flat line is a dead
   * reader, not a still animal, and the plot must be able to say which. */
  stale: boolean;
}

/** Points for one axis, keyed `<device>.<axis>`. */
export type Traces = Map<string, TracePoint[]>;

/** ~2 minutes at the snapshot rate: enough for the longest window, bounded. */
export const MAX_POINTS = 4000;

/**
 * The span a derived speed is measured over.
 *
 * Not the gap between two snapshots. The value arrives quantised — a wheel
 * publishes whole counts, and the stream's own timing jitters by a millisecond
 * or two — so a slope taken across one ~33 ms step is mostly that quantisation,
 * and it draws a steady run as a hedge of spikes. Over a quarter of a second
 * the artefact averages out while a real start or stop is still immediate to
 * the eye. It is a stated window, not a smoothing filter: the panel says so.
 */
export const SPEED_SPAN_S = 0.25;

export function traceKey(device: string, axis: string): string {
  return `${device}.${axis}`;
}

/**
 * Append this snapshot's readings, in place, dropping what has aged out.
 *
 * Speed is the slope across [`SPEED_SPAN_S`] for an absolute or cumulative
 * axis, and the reading itself for a rate axis — which already *is* a speed,
 * and whose integral is not a position this client may invent.
 *
 * A stale reading still produces a point: the device is holding its last value,
 * which is a fact worth drawing. Its slope is not, so it is reported as zero
 * rather than as whatever the arithmetic says about a value that did not move.
 */
export function appendSnapshot(traces: Traces, devices: InputDeviceView[], t: number): void {
  for (const device of devices) {
    for (const axis of device.axes) {
      const key = traceKey(device.name, axis.name);
      const points = traces.get(key) ?? [];
      const reference = spanStart(points, t - SPEED_SPAN_S);
      const dt = reference ? t - reference.t : 0;
      const speed =
        axis.semantic === "rate"
          ? axis.value
          : device.stale || !reference || dt <= 0
            ? 0
            : (axis.value - reference.value) / dt;
      points.push({ t, value: axis.value, speed, stale: device.stale });
      if (points.length > MAX_POINTS) points.splice(0, points.length - MAX_POINTS);
      traces.set(key, points);
    }
  }
}

/**
 * The newest point at or before `from` — the other end of the speed span.
 *
 * Falls back to the oldest point there is, so a trace shorter than the span
 * still reports a slope (over a shorter base) rather than nothing at all.
 */
function spanStart(points: TracePoint[], from: number): TracePoint | undefined {
  for (let i = points.length - 1; i >= 0; i--) {
    if (points[i].t <= from) return points[i];
  }
  return points[0];
}

/** The points within `windowS` of the newest one, oldest first. */
export function windowed(points: TracePoint[], windowS: number): TracePoint[] {
  const newest = points[points.length - 1];
  if (!newest) return [];
  const from = newest.t - windowS;
  const i = points.findIndex((p) => p.t >= from);
  return i <= 0 ? points : points.slice(i);
}

export interface Domain {
  min: number;
  max: number;
}

/**
 * The value range to draw, padded so a trace never touches the frame.
 *
 * `includeZero` is true for speed: a speed chart that scales to 40–42 cm/s
 * shows a dramatic wobble where the animal ran at a steady pace, and one that
 * hides the zero line cannot show it stopping. Position autoscales instead —
 * a cumulative axis is tens of metres in, and its zero is not on the screen.
 */
export function domainOf(values: number[], includeZero: boolean): Domain {
  let min = includeZero ? 0 : Infinity;
  let max = includeZero ? 0 : -Infinity;
  for (const v of values) {
    if (!Number.isFinite(v)) continue;
    min = Math.min(min, v);
    max = Math.max(max, v);
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) return { min: 0, max: 1 };
  if (max - min < 1e-9) return { min: min - 1, max: max + 1 };
  const pad = (max - min) * 0.08;
  // Pad away from zero only. A speed chart whose baseline sits a little below
  // zero shows an animal at rest hovering above the floor, and the zero line —
  // the one value on that chart that means something on its own — lands
  // somewhere arbitrary instead of on the edge of the plot.
  return {
    min: includeZero && min >= 0 ? 0 : min - pad,
    max: includeZero && max <= 0 ? 0 : max + pad,
  };
}

/** Three tick values across a domain: the ends and the middle. */
export function ticksOf({ min, max }: Domain): number[] {
  return [max, (min + max) / 2, min];
}

/** Enough decimals to tell two neighbouring ticks apart, and no more. */
export function decimalsFor(span: number): number {
  if (span >= 100) return 0;
  if (span >= 10) return 1;
  if (span >= 1) return 2;
  return 3;
}

/**
 * Split into runs of equal staleness, each keeping the point before it so the
 * line has no gap where the state changes.
 */
export function staleRuns(points: TracePoint[]): { stale: boolean; points: TracePoint[] }[] {
  const runs: { stale: boolean; points: TracePoint[] }[] = [];
  for (const p of points) {
    const last = runs[runs.length - 1];
    if (!last || last.stale !== p.stale) {
      const bridge = last ? [last.points[last.points.length - 1]] : [];
      runs.push({ stale: p.stale, points: [...bridge, p] });
    } else {
      last.points.push(p);
    }
  }
  return runs;
}

/** The point nearest `t`, for the hover readout. */
export function nearest(points: TracePoint[], t: number): TracePoint | undefined {
  let best: TracePoint | undefined;
  let bestDistance = Infinity;
  for (const p of points) {
    const d = Math.abs(p.t - t);
    if (d < bestDistance) {
      best = p;
      bestDistance = d;
    }
  }
  return best;
}
