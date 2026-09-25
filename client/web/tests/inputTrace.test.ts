// The input-device trace maths, without a DOM: what the plot in InputPanel is
// actually claiming about a wheel.

import { describe, expect, it } from "vitest";

import type { InputDeviceView } from "../src/index.js";
import {
  appendSnapshot,
  domainOf,
  nearest,
  staleRuns,
  traceKey,
  windowed,
  type TracePoint,
  type Traces,
} from "../src/app/inputTrace.js";

function device(overrides: Partial<InputDeviceView> & { axes: InputDeviceView["axes"] }): InputDeviceView {
  return {
    name: "treadmill",
    backend: "shm /vstimd_wheel",
    connected: true,
    stale: false,
    tornReads: 0n,
    starvedFrames: 0n,
    ...overrides,
  };
}

const cumulative = (value: number, stale = false) =>
  device({ stale, axes: [{ name: "distance", semantic: "cumulative", value, delta: 0 }] });

describe("appendSnapshot", () => {
  it("derives speed from the slope between snapshots", () => {
    const traces: Traces = new Map();
    appendSnapshot(traces, [cumulative(10)], 100);
    appendSnapshot(traces, [cumulative(15)], 100.25);
    const points = traces.get(traceKey("treadmill", "distance"))!;
    expect(points).toHaveLength(2);
    expect(points[0].speed).toBe(0); // no previous point to difference against
    expect(points[1].speed).toBeCloseTo(20, 9); // 5 units in 250 ms
  });

  it("measures speed across the span, so one quantised step is not a spike", () => {
    // A wheel publishing whole counts delivers its motion in steps: some 33 ms
    // frames see two counts and the next sees none. Across one frame that is a
    // hedge of spikes; across the span it is the 10 units/s the animal ran.
    const traces: Traces = new Map();
    let value = 0;
    for (let i = 0; i <= 20; i++) {
      // Alternating 0 and 2 units per 100 ms step: 10 units/s, delivered lumpily.
      if (i > 0) value += i % 2 === 0 ? 2 : 0;
      appendSnapshot(traces, [cumulative(value)], i * 0.1);
    }
    const points = traces.get(traceKey("treadmill", "distance"))!;
    const recent = points.slice(-6).map((p) => p.speed);
    for (const speed of recent) expect(speed).toBeGreaterThan(6);
    for (const speed of recent) expect(speed).toBeLessThan(14);
  });

  it("reports no speed while the producer is stale", () => {
    // The value is held, not moving: a slope computed from it would be a lie
    // about an animal that may well be running on a wheel nobody is reading.
    const traces: Traces = new Map();
    appendSnapshot(traces, [cumulative(10)], 100);
    appendSnapshot(traces, [cumulative(10, true)], 100.5);
    const points = traces.get(traceKey("treadmill", "distance"))!;
    expect(points[1]).toMatchObject({ value: 10, speed: 0, stale: true });
  });

  it("takes a rate axis' reading as the speed rather than differencing it", () => {
    const traces: Traces = new Map();
    const stick = (value: number) =>
      device({ name: "stick", axes: [{ name: "speed", semantic: "rate", value, delta: 0 }] });
    appendSnapshot(traces, [stick(4)], 1);
    appendSnapshot(traces, [stick(4)], 2);
    expect(traces.get(traceKey("stick", "speed"))!.map((p) => p.speed)).toEqual([4, 4]);
  });

  it("keeps every axis of every device apart", () => {
    const traces: Traces = new Map();
    appendSnapshot(
      traces,
      [
        device({ axes: [{ name: "distance", semantic: "cumulative", value: 1, delta: 0 }] }),
        device({
          name: "gaze",
          axes: [
            { name: "x", semantic: "absolute", value: 2, delta: 0 },
            { name: "y", semantic: "absolute", value: 3, delta: 0 },
          ],
        }),
      ],
      1,
    );
    expect([...traces.keys()]).toEqual(["treadmill.distance", "gaze.x", "gaze.y"]);
  });
});

describe("windowed", () => {
  const points: TracePoint[] = [0, 1, 2, 3, 4].map((t) => ({ t, value: t, speed: 1, stale: false }));

  it("keeps the window ending at the newest point", () => {
    expect(windowed(points, 2).map((p) => p.t)).toEqual([2, 3, 4]);
  });

  it("keeps everything when the window is longer than the trace", () => {
    expect(windowed(points, 60)).toHaveLength(5);
    expect(windowed([], 5)).toEqual([]);
  });
});

describe("domainOf", () => {
  it("includes zero for speed, and not for position", () => {
    // Exactly zero, not zero minus padding: on a speed chart the baseline is
    // "stopped", and it belongs on the floor of the plot.
    expect(domainOf([40, 42], true).min).toBe(0);
    expect(domainOf([40, 42], false).min).toBeGreaterThan(39);
  });

  it("pads away from zero, on whichever side the data is", () => {
    expect(domainOf([-5, -1], true)).toMatchObject({ max: 0 });
    const both = domainOf([-5, 5], true);
    expect(both.min).toBeLessThan(-5);
    expect(both.max).toBeGreaterThan(5);
  });

  it("gives a flat trace a range to be drawn in", () => {
    const { min, max } = domainOf([7, 7, 7], false);
    expect(max - min).toBeGreaterThan(0);
  });

  it("survives an empty trace", () => {
    expect(domainOf([], false)).toEqual({ min: 0, max: 1 });
  });
});

describe("staleRuns", () => {
  it("splits at each change and bridges the gap", () => {
    const points: TracePoint[] = [
      { t: 0, value: 0, speed: 0, stale: false },
      { t: 1, value: 1, speed: 1, stale: false },
      { t: 2, value: 1, speed: 0, stale: true },
      { t: 3, value: 5, speed: 4, stale: false },
    ];
    const runs = staleRuns(points);
    expect(runs.map((r) => r.stale)).toEqual([false, true, false]);
    // Each later run starts on the previous run's last point, so the line is
    // continuous across the change instead of showing a hole.
    expect(runs[1].points.map((p) => p.t)).toEqual([1, 2]);
    expect(runs[2].points.map((p) => p.t)).toEqual([2, 3]);
  });
});

describe("nearest", () => {
  it("finds the sample under the cursor", () => {
    const points: TracePoint[] = [0, 1, 2].map((t) => ({ t, value: t, speed: 0, stale: false }));
    expect(nearest(points, 1.4)?.t).toBe(1);
    expect(nearest(points, 1.6)?.t).toBe(2);
    expect(nearest([], 1)).toBeUndefined();
  });
});
