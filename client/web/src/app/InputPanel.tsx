// The rig's input devices — a wheel, a treadmill, an eye tracker — as state and
// as a trace: where each axis is, and how fast it is moving, over the last few
// seconds.
//
// Why a plot and not a number: the three things that go wrong with a position
// device are invisible in a single reading. A reader that died leaves a number
// that looks fine and never changes; a miscalibrated wheel reads plausibly and
// is wrong by a factor; a producer slower than the display makes motion that
// stutters while every individual sample is correct. All three are obvious in
// ten seconds of trace, and the first one is why the stale span is drawn in a
// different ink rather than as a flat line like any other.
//
// Position and speed are two charts and never two y-scales on one: a dual axis
// invites reading a crossing as an event when it is an artefact of two
// arbitrary scales. They share a time base and a hover, so reading them at one
// instant is what the pair is for.

import { useEffect, useRef, useState, type PointerEvent } from "react";

import type { Connection, InputDeviceView, SceneSnapshot } from "../index.js";
import {
  type Domain,
  type TracePoint,
  type Traces,
  appendSnapshot,
  decimalsFor,
  domainOf,
  nearest,
  staleRuns,
  ticksOf,
  traceKey,
  windowed,
} from "./inputTrace.js";

interface Props {
  conn: Connection | null;
  snapshot: SceneSnapshot | null;
}

/** Validated against this panel's surface — see the dataviz palette, dark steps. */
const POSITION = "#3987e5";
const SPEED = "#d95926";
const SURFACE = "#1a1a1a";
const MUTED_MARK = "#6b6b6b"; // a stale span: present, and not the series' own ink
const GRID = "#2e2e2e";
const ZERO_LINE = "#454545";
const INK = "#ddd";
const INK_MUTED = "#888";

const WINDOWS_S = [5, 15, 60];

const W = 340;
const H = 58;
const PAD_LEFT = 46;
const PAD_RIGHT = 10;
const PAD_Y = 7;
const PLOT_W = W - PAD_LEFT - PAD_RIGHT;
const PLOT_H = H - 2 * PAD_Y;

interface ChartProps {
  points: TracePoint[];
  /** The newest point's time: the right edge is always "now". */
  now: number;
  windowS: number;
  pick: (p: TracePoint) => number;
  color: string;
  /** Speed charts always show zero; position charts autoscale to the window. */
  includeZero: boolean;
  hoverT: number | null;
  onHover: (t: number | null) => void;
}

function Chart({ points, now, windowS, pick, color, includeZero, hoverT, onHover }: ChartProps) {
  const domain: Domain = domainOf(points.map(pick), includeZero);
  const t0 = now - windowS;
  const x = (t: number) => PAD_LEFT + Math.max(0, Math.min(1, (t - t0) / windowS)) * PLOT_W;
  const y = (v: number) => PAD_Y + PLOT_H - ((v - domain.min) / (domain.max - domain.min)) * PLOT_H;
  const path = (run: TracePoint[]) =>
    run.map((p, i) => `${i ? "L" : "M"}${x(p.t).toFixed(1)},${y(pick(p)).toFixed(1)}`).join(" ");

  const decimals = decimalsFor(domain.max - domain.min);
  const newest = points[points.length - 1];
  const hovered = hoverT === null ? undefined : nearest(points, hoverT);

  // The pointer's own x, not the hovered sample's: a hairline that snaps
  // between samples reads as a stutter in the data.
  const track = (e: PointerEvent<SVGSVGElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - box.left) / box.width) * W;
    onHover(t0 + ((px - PAD_LEFT) / PLOT_W) * windowS);
  };

  return (
    <svg
      width={W}
      height={H}
      style={{ display: "block", touchAction: "none" }}
      onPointerMove={track}
      onPointerDown={track}
      onPointerLeave={() => onHover(null)}
    >
      {ticksOf(domain).map((v) => (
        <g key={v}>
          <line
            x1={PAD_LEFT}
            x2={W - PAD_RIGHT}
            y1={y(v)}
            y2={y(v)}
            stroke={includeZero && Math.abs(v) < 1e-9 ? ZERO_LINE : GRID}
            strokeWidth={1}
          />
          <text x={PAD_LEFT - 6} y={y(v) + 3} textAnchor="end" fontSize={9} fill={INK_MUTED}>
            {v.toFixed(decimals)}
          </text>
        </g>
      ))}
      {includeZero && domain.min < 0 && domain.max > 0 && (
        <line x1={PAD_LEFT} x2={W - PAD_RIGHT} y1={y(0)} y2={y(0)} stroke={ZERO_LINE} strokeWidth={1} />
      )}

      {staleRuns(points).map((run, i) => (
        <path
          key={i}
          d={path(run.points)}
          fill="none"
          stroke={run.stale ? MUTED_MARK : color}
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      ))}

      {newest && (
        <circle
          cx={x(newest.t)}
          cy={y(pick(newest))}
          r={3}
          fill={newest.stale ? MUTED_MARK : color}
          stroke={SURFACE}
          strokeWidth={2}
        />
      )}

      {hoverT !== null && hovered && (
        <>
          <line
            x1={x(hoverT)}
            x2={x(hoverT)}
            y1={PAD_Y}
            y2={PAD_Y + PLOT_H}
            stroke={INK_MUTED}
            strokeWidth={1}
          />
          <circle
            cx={x(hovered.t)}
            cy={y(pick(hovered))}
            r={3.5}
            fill={hovered.stale ? MUTED_MARK : color}
            stroke={SURFACE}
            strokeWidth={2}
          />
        </>
      )}
    </svg>
  );
}

/**
 * One axis: its charts, their shared hover, and the readout above them.
 *
 * A rate axis gets **one** chart. Its reading already is a speed, so a position
 * chart would either repeat the same line under a wrong name or plot an
 * integral this client invented — and an invented position is exactly the thing
 * a device-driven stimulus must not be steered by.
 */
function AxisTraces({
  points,
  windowS,
  hasPosition,
}: {
  points: TracePoint[];
  windowS: number;
  hasPosition: boolean;
}) {
  const [hoverT, setHoverT] = useState<number | null>(null);
  const newest = points[points.length - 1];
  if (!newest) return <div style={{ color: INK_MUTED, fontSize: 12 }}>waiting for the first sample…</div>;

  const hovered = hoverT === null ? undefined : nearest(points, hoverT);
  const shown = hovered ?? newest;
  const ago = newest.t - shown.t;

  const readout = (label: string, value: number, color: string) => (
    <div style={{ display: "flex", alignItems: "baseline", gap: 6 }}>
      <svg width={10} height={10} aria-hidden="true">
        <circle cx={5} cy={5} r={3} fill={color} />
      </svg>
      <span style={{ color: INK_MUTED, fontSize: 11 }}>{label}</span>
      <span style={{ color: INK, fontFamily: "monospace", fontSize: 12 }}>{value.toFixed(2)}</span>
    </div>
  );

  return (
    <div>
      <div style={{ display: "flex", gap: 14, alignItems: "baseline", flexWrap: "wrap", marginBottom: 2 }}>
        {hasPosition && readout("position", shown.value, POSITION)}
        {readout("per second", shown.speed, SPEED)}
        <span style={{ color: INK_MUTED, fontSize: 11, marginLeft: "auto" }}>
          {hovered ? `${ago.toFixed(1)} s ago` : "live"}
        </span>
      </div>
      {hasPosition && (
        <Chart
          points={points}
          now={newest.t}
          windowS={windowS}
          pick={(p) => p.value}
          color={POSITION}
          includeZero={false}
          hoverT={hoverT}
          onHover={setHoverT}
        />
      )}
      <div style={{ height: 4 }} />
      <Chart
        points={points}
        now={newest.t}
        windowS={windowS}
        pick={(p) => p.speed}
        color={SPEED}
        includeZero
        hoverT={hoverT}
        onHover={setHoverT}
      />
      <div style={{ display: "flex", justifyContent: "space-between", color: INK_MUTED, fontSize: 10 }}>
        <span>−{windowS} s</span>
        <span>now</span>
      </div>
    </div>
  );
}

function DeviceTraces({
  device,
  traces,
  windowS,
}: {
  device: InputDeviceView;
  traces: Traces;
  windowS: number;
}) {
  const [color, state] = !device.connected
    ? ["#e66", "not connected"]
    : device.stale
      ? ["#e66", "STALE"]
      : ["#6c6", "live"];
  // A shm producer is the rig; a keyboard or gamepad is somebody's desk, and
  // nobody should discover mid-session that the wheel was the arrow keys.
  const overridden = !device.backend.startsWith("shm ");

  return (
    <div style={{ marginBottom: 14 }}>
      <div style={{ display: "flex", gap: 6, alignItems: "baseline", flexWrap: "wrap", fontSize: 13 }}>
        <strong>{device.name}</strong>
        <span style={{ color: overridden ? "#dc6" : INK_MUTED }}>{device.backend}</span>
        <span style={{ color }}>{state}</span>
        {device.tornReads > 0n && <span style={{ color: INK_MUTED }}>torn {String(device.tornReads)}</span>}
        {device.starvedFrames > 0n && (
          <span
            style={{ color: "#dc6" }}
            title="Frames that found no new sample. A climbing count means the producer samples at or below the display rate, which is motion that stutters."
          >
            no-sample frames {String(device.starvedFrames)}
          </span>
        )}
      </div>
      {device.axes.map((axis) => (
        <div key={axis.name} style={{ marginTop: 4 }}>
          <div style={{ color: INK_MUTED, fontSize: 11 }}>
            {axis.name} · {axis.semantic}
            {axis.semantic === "rate" && " — the reading is a speed, so there is no position to plot"}
          </div>
          <AxisTraces
            points={windowed(traces.get(traceKey(device.name, axis.name)) ?? [], windowS)}
            windowS={windowS}
            hasPosition={axis.semantic !== "rate"}
          />
        </div>
      ))}
    </div>
  );
}

export function InputPanel({ snapshot }: Props) {
  const traces = useRef<Traces>(new Map());
  const lastTimeNs = useRef<bigint | null>(null);
  const [, setTick] = useState(0);
  const [windowS, setWindowS] = useState(15);

  const devices = snapshot?.inputDevices ?? [];
  useEffect(() => {
    if (!snapshot || snapshot.inputDevices.length === 0) return;
    // One append per snapshot: an effect can run twice for one value (React
    // StrictMode does exactly that in development), and a doubled point is a
    // dt of zero and a speed of zero in the middle of a moving trace.
    if (lastTimeNs.current === snapshot.serverTimeNs) return;
    lastTimeNs.current = snapshot.serverTimeNs;
    appendSnapshot(traces.current, snapshot.inputDevices, Number(snapshot.serverTimeNs) / 1e9);
    setTick((n) => n + 1);
  }, [snapshot]);

  return (
    <div style={{ minWidth: W + 8 }}>
      <h3>Input</h3>
      {devices.length === 0 ? (
        <p style={{ color: INK_MUTED, fontSize: 12, maxWidth: W }}>
          This rig declares no input devices. They are <code>[[input.device]]</code> entries in the
          rig config, not something a client creates.
        </p>
      ) : (
        <>
          <div style={{ display: "flex", gap: 6, alignItems: "baseline", marginBottom: 8 }}>
            <span style={{ color: INK_MUTED, fontSize: 11 }}>window</span>
            {WINDOWS_S.map((s) => (
              <button
                key={s}
                onClick={() => setWindowS(s)}
                style={{
                  fontSize: 11,
                  padding: "1px 6px",
                  color: s === windowS ? INK : INK_MUTED,
                  background: s === windowS ? "#3a3a3a" : "transparent",
                  border: `1px solid ${s === windowS ? "#5a5a5a" : "#333"}`,
                  borderRadius: 3,
                  cursor: "pointer",
                }}
              >
                {s} s
              </button>
            ))}
          </div>
          {devices.map((d) => (
            <DeviceTraces key={d.name} device={d} traces={traces.current} windowS={windowS} />
          ))}
          <p style={{ color: "#666", fontSize: 11, maxWidth: W, marginTop: 0 }}>
            Sampled from the state stream at ~30 Hz, in each axis' own units, with speed as the
            slope over the last 250 ms — a monitor, not a record. vstimd reads the device every
            frame and its producer writes faster still; the per-sample trace belongs to whatever
            owns the device.
          </p>
        </>
      )}
    </div>
  );
}
