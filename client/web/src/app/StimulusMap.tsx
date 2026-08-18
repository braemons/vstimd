// Interactive map: a scaled vector view of the screen reconstructed from the
// snapshot (not a frame stream). Drag a stimulus to move it — the core of manual
// receptive-field mapping. Drags update an optimistic local position immediately
// and send setPosition coalesced to one message per animation frame; the next
// snapshot reconciles.

import { useEffect, useRef, useState } from "react";
import type { Connection, SceneSnapshot, StimulusView, Vec2 } from "../index.js";

interface Props {
  conn: Connection | null;
  snapshot: SceneSnapshot | null;
}

const FALLBACK = { widthPx: 1920, heightPx: 1080 };

export function StimulusMap({ conn, snapshot }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Optimistic override for the stimulus currently being dragged.
  const dragRef = useRef<{ handle: number; posPx: Vec2 } | null>(null);
  const pendingRef = useRef<Vec2 | null>(null);
  const rafRef = useRef<number | null>(null);
  const [, force] = useState(0);

  const screen = snapshot?.serverInfo
    ? { widthPx: snapshot.serverInfo.widthPx || FALLBACK.widthPx, heightPx: snapshot.serverInfo.heightPx || FALLBACK.heightPx }
    : FALLBACK;

  // Canvas <-> stimulus-space transforms (origin centre, +y up).
  function geom(canvas: HTMLCanvasElement) {
    const scale = Math.min(canvas.width / screen.widthPx, canvas.height / screen.heightPx);
    const cx = canvas.width / 2;
    const cy = canvas.height / 2;
    return {
      toCanvas: (p: Vec2) => ({ x: cx + p.x * scale, y: cy - p.y * scale }),
      toStimulus: (x: number, y: number): Vec2 => ({ x: (x - cx) / scale, y: (cy - y) / scale }),
      scale,
    };
  }

  function stimuli(): StimulusView[] {
    const list = snapshot?.stimuli ?? [];
    const drag = dragRef.current;
    if (!drag) return list;
    return list.map((s) => (s.handle === drag.handle ? { ...s, posPx: drag.posPx } : s));
  }

  // Redraw whenever the snapshot or drag changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { toCanvas, scale } = geom(canvas);

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    // screen border
    const tl = toCanvas({ x: -screen.widthPx / 2, y: screen.heightPx / 2 });
    ctx.strokeStyle = "#444";
    ctx.strokeRect(tl.x, tl.y, screen.widthPx * scale, screen.heightPx * scale);

    for (const s of stimuli()) {
      const c = toCanvas(s.posPx);
      const col = s.fillColor;
      const css = col ? `rgba(${col.r * 255},${col.g * 255},${col.b * 255},${s.enabled ? col.a : 0.25})` : "#888";
      // True-to-scale: stimulus pixels → canvas pixels. Keep a small floor so
      // tiny stimuli stay clickable/visible.
      const w = Math.max(4, s.size.widthPx * scale);
      const h = Math.max(4, s.size.heightPx * scale);

      ctx.save();
      ctx.translate(c.x, c.y);
      ctx.rotate((-s.rotationDeg * Math.PI) / 180); // +y up → canvas +y down
      ctx.fillStyle = css;
      ctx.strokeStyle = "#fff";
      ctx.lineWidth = 1;
      if (s.type === "circle" || s.type === "ellipse") {
        ctx.beginPath();
        ctx.ellipse(0, 0, w / 2, h / 2, 0, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      } else {
        ctx.fillRect(-w / 2, -h / 2, w, h);
        ctx.strokeRect(-w / 2, -h / 2, w, h);
      }
      ctx.restore();

      ctx.fillStyle = "#ccc";
      ctx.font = "11px sans-serif";
      ctx.fillText(s.name || s.type, c.x + w / 2 + 4, c.y + 4);
    }
  });

  function hitTest(sx: number, sy: number): StimulusView | null {
    const canvas = canvasRef.current!;
    const { toCanvas, scale } = geom(canvas);
    // topmost (last drawn) first; axis-aligned bbox with a clickable floor.
    const list = stimuli();
    for (let i = list.length - 1; i >= 0; i--) {
      const c = toCanvas(list[i].posPx);
      const hw = Math.max(8, (list[i].size.widthPx * scale) / 2);
      const hh = Math.max(8, (list[i].size.heightPx * scale) / 2);
      if (Math.abs(sx - c.x) <= hw && Math.abs(sy - c.y) <= hh) return list[i];
    }
    return null;
  }

  function flush() {
    rafRef.current = null;
    const drag = dragRef.current;
    const posPx = pendingRef.current;
    if (drag && posPx && conn) {
      conn.stimuli.setPosition(drag.handle, posPx).catch(() => {});
      pendingRef.current = null;
    }
  }

  function onPointerDown(e: React.PointerEvent<HTMLCanvasElement>) {
    const rect = e.currentTarget.getBoundingClientRect();
    const hit = hitTest(e.clientX - rect.left, e.clientY - rect.top);
    if (!hit) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    dragRef.current = { handle: hit.handle, posPx: hit.posPx };
    force((n) => n + 1);
  }

  function onPointerMove(e: React.PointerEvent<HTMLCanvasElement>) {
    if (!dragRef.current) return;
    const canvas = e.currentTarget;
    const rect = canvas.getBoundingClientRect();
    const { toStimulus } = geom(canvas);
    const posPx = toStimulus(e.clientX - rect.left, e.clientY - rect.top);
    dragRef.current = { handle: dragRef.current.handle, posPx };
    pendingRef.current = posPx;
    if (rafRef.current == null) rafRef.current = requestAnimationFrame(flush);
    force((n) => n + 1);
  }

  function onPointerUp(e: React.PointerEvent<HTMLCanvasElement>) {
    if (!dragRef.current) return;
    flush();
    dragRef.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
    force((n) => n + 1);
  }

  return (
    <canvas
      ref={canvasRef}
      width={960}
      height={540}
      style={{ background: "#111", border: "1px solid #333", touchAction: "none", cursor: "crosshair" }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    />
  );
}
