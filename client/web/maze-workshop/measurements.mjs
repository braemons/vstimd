import { isLandmark, isRoundLandmark, landmarkFootprint } from './landmarks.mjs';
import { corners } from './model.mjs';
const length = (a, b) => Math.hypot(b[0] - a[0], b[1] - a[1]);
const cm = value => `${Number(value.toFixed(2))} cm`;
function footprint(o) { return corners(isLandmark(o) ? landmarkFootprint(o) : o.kind === 'arch' ? { ...o, w: o.w + 8 } : o); }
// Distance from a centre to its boundary along a given unit direction.
function radiusAlong(o, ux, us) {
    if (isRoundLandmark(o))
        return o.w / 2;
    const a = o.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
    const u = Math.abs(ux * c + us * s), v = Math.abs(-ux * s + us * c);
    return Math.min(u > 1e-12 ? (o.w + (o.kind === 'arch' ? 8 : 0)) / 2 / u : Infinity, v > 1e-12 ? o.d / 2 / v : Infinity);
}
export function planMeasurements(objects, selectedIds = []) {
    if (!objects.length)
        return [];
    const segments = [], selected = new Set(selectedIds), chosen = objects.filter(o => selected.has(o.id));
    const add = (from, to, label, offset = 18, type = 'size', priority = false) => segments.push({ from, to, label, offset, type, priority });
    const all = objects.flatMap(footprint), xs = all.map(p => p[0]), ss = all.map(p => p[1]);
    const x0 = Math.min(...xs), x1 = Math.max(...xs), s0 = Math.min(...ss), s1 = Math.max(...ss);
    add([x0, s0], [x1, s0], cm(x1 - x0), 36, 'overall', true);
    add([x0, s1], [x0, s0], cm(s1 - s0), 36, 'overall', true);
    for (const o of chosen.length ? chosen : objects) {
        const points = corners({ ...o, d: o.kind === 'sphere' ? o.w : o.d, angle: o.kind === 'sphere' ? 0 : o.angle });
        add(points[0], points[1], isRoundLandmark(o) ? `Ø ${cm(o.w)}` : cm(o.w), 18, 'size', selected.has(o.id));
        if (!isRoundLandmark(o))
            add(points[3], points[0], cm(o.d), 18, 'size', selected.has(o.id));
    }
    if (chosen.length >= 2) {
        let a = chosen[0], b = chosen[1], max = 0;
        for (let i = 0; i < chosen.length; i++)
            for (let j = i + 1; j < chosen.length; j++) {
                const d = length([chosen[i].x, chosen[i].s], [chosen[j].x, chosen[j].s]);
                if (d > max) {
                    max = d;
                    a = chosen[i];
                    b = chosen[j];
                }
            }
        if (max) {
            const ux = (b.x - a.x) / max, us = (b.s - a.s) / max;
            const row = [...chosen].sort((a, b) => (a.x - b.x) * ux + (a.s - b.s) * us);
            for (let i = 1; i < row.length; i++) {
                const a = row[i - 1], b = row[i], distance = Math.hypot(b.x - a.x, b.s - a.s);
                if (distance < 1e-8)
                    continue;
                const dx = (b.x - a.x) / distance, ds = (b.s - a.s) / distance, ra = radiusAlong(a, dx, ds), rb = radiusAlong(b, dx, ds);
                const gap = distance - ra - rb;
                if (gap >= 0)
                    add([a.x + ra * dx, a.s + ra * ds], [b.x - rb * dx, b.s - rb * ds], `${cm(gap)} gap`, -24, 'gap', true);
                else
                    add([a.x, a.s], [b.x, b.s], `${cm(distance)} centres`, -24, 'position', true);
            }
        }
    }
    return segments;
}
export function rulerDistance(from, to) { return Math.hypot(to.x - from.x, to.s - from.s); }
export function snapRulerPoint(objects, point, tolerance_cm) {
    let corner = null, cornerDistance = tolerance_cm;
    for (const o of objects)
        if (!isRoundLandmark(o))
            for (const p of footprint(o)) {
                const d = Math.hypot(point.x - p[0], point.s - p[1]);
                if (d < cornerDistance) {
                    cornerDistance = d;
                    corner = { x: p[0], s: p[1] };
                }
            }
    if (corner)
        return { ...corner, snapped: true };
    let closest = null, best = tolerance_cm;
    const candidate = (x, s) => { const d = Math.hypot(point.x - x, point.s - s); if (d < best) {
        best = d;
        closest = { x, s };
    } };
    for (const o of objects) {
        if (isRoundLandmark(o)) {
            const dx = point.x - o.x, ds = point.s - o.s, d = Math.hypot(dx, ds);
            if (d > 1e-10)
                candidate(o.x + dx / d * o.w / 2, o.s + ds / d * o.w / 2);
        }
        else {
            const points = footprint(o);
            for (let i = 0; i < 4; i++) {
                const a = points[i], b = points[(i + 1) % 4], dx = b[0] - a[0], ds = b[1] - a[1], den = dx * dx + ds * ds;
                const t = den ? Math.max(0, Math.min(1, ((point.x - a[0]) * dx + (point.s - a[1]) * ds) / den)) : 0;
                candidate(a[0] + dx * t, a[1] + ds * t);
            }
        }
    }
    return closest ? { ...closest, snapped: true } : { x: point.x, s: point.s, snapped: false };
}
