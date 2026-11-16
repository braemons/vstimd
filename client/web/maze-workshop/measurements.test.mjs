import test from 'node:test';
import assert from 'node:assert/strict';
import { object } from './model.mjs';
import { planMeasurements, rulerDistance, snapRulerPoint } from './measurements.mjs';
const camera = { x: 0, s: 0 };
test('plan dimensions measure rotated geometry, retain cm and do not change the project', () => {
    const wall = object('wall', { x: 50, s: 100, w: 120, d: 4, angle: 37 }), before = JSON.stringify(wall);
    const segments = planMeasurements([wall], [wall.id]);
    const sizes = segments.filter(s => s.type === 'size');
    assert.equal(sizes[0].label, '120 cm');
    assert.equal(sizes[1].label, '4 cm');
    assert.ok(Math.abs(Math.hypot(sizes[0].to[0] - sizes[0].from[0], sizes[0].to[1] - sizes[0].from[1]) - 120) < 1e-8);
    assert.equal(JSON.stringify(wall), before);
});
test('selected panels show the actual clear gap between their facing edges', () => {
    const panels = [0, 20, 40].map(s => object('wall', { x: 22, s, w: .2, d: 10 }));
    const segments = planMeasurements(panels, panels.map(o => o.id));
    const gaps = segments.filter(s => s.type === 'gap');
    assert.equal(gaps.length, 2);
    for (const gap of gaps) {
        assert.equal(gap.label, '10 cm gap');
        assert.equal(Math.hypot(gap.to[0] - gap.from[0], gap.to[1] - gap.from[1]), 10);
    }
});
test('selecting a landmark shows its diameter without any automatic camera distance', () => {
    const sphere = object('sphere', { x: 30, s: 40, w: 12 });
    const segments = planMeasurements([sphere], [sphere.id]);
    assert.equal(segments.some(s => s.type === 'position'), false);
    assert.equal(segments.some(s => s.label.includes('camera')), false);
    assert.equal(segments.find(s => s.type === 'size').label, 'Ø 12 cm');
});
test('rotated clearances follow the centre line; overlapping objects use centre distance', () => {
    const c = Math.SQRT1_2;
    const panels = [0, 20].map(t => object('wall', { x: t * c, s: t * c, w: 10, d: 2, angle: 45 }));
    const gaps = planMeasurements(panels, panels.map(o => o.id)).filter(s => s.type === 'gap');
    assert.equal(gaps[0].label, '10 cm gap');
    const overlap = [object('wall', { x: 0, s: 0, w: 20 }), object('wall', { x: 5, s: 0, w: 20 })];
    assert.equal(planMeasurements(overlap, overlap.map(o => o.id)).find(s => s.type === 'position').label, '5 cm centres');
    assert.deepEqual(planMeasurements([]), []);
});
test('ruler measures point-to-point distances in cm, including diagonals and zero', () => {
    assert.equal(rulerDistance({ x: 0, s: 0 }, { x: 30, s: 40 }), 50);
    assert.equal(rulerDistance({ x: 10, s: 20 }, { x: 10, s: 20 }), 0);
    assert.equal(rulerDistance({ x: -20, s: 100 }, { x: 20, s: 100 }), 40);
});
test('ruler snaps to corners, wall edges and spherical edges without grid rounding', () => {
    const wall = object('wall', { x: 0, s: 0, w: 100, d: 4 });
    assert.deepEqual(snapRulerPoint([wall], { x: 49.9, s: 1.9 }, .5), { x: 50, s: 2, snapped: true });
    const edge = snapRulerPoint([wall], { x: 12.34, s: 2.1 }, .5);
    assert.ok(Math.abs(edge.x - 12.34) < 1e-10);
    assert.equal(edge.s, 2);
    assert.equal(edge.snapped, true);
    assert.deepEqual(snapRulerPoint([wall], { x: 0, s: 30 }, .5), { x: 0, s: 30, snapped: false });
    const sphere = object('sphere', { x: 0, s: 0, w: 20 });
    assert.deepEqual(snapRulerPoint([sphere], { x: 10.1, s: 0 }, .5), { x: 10, s: 0, snapped: true });
    const rotated = object('wall', { x: 0, s: 0, w: 40, d: 4, angle: 90 });
    const p = snapRulerPoint([rotated], { x: 2.1, s: 10 }, .5);
    assert.ok(Math.abs(p.x - 2) < 1e-8);
    assert.ok(Math.abs(p.s - 10) < 1e-8);
});
