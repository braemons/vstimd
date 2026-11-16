import test from 'node:test';
import assert from 'node:assert/strict';
import { minimapTransform, worldToMinimap, minimapToWorld, previewOffsets } from './navigation.mjs';
import { landmarkHeight, landmarkElevation, landmarkAtElevation } from './landmarks.mjs';
import { object, template, exportScene, solids } from './model.mjs';
test('compact horizontal map preserves aspect ratio and maps forward to the right', () => {
    const map = minimapTransform({ minX: -20, maxX: 20, minS: 0, maxS: 400 }, 180, 110);
    const near = worldToMinimap({ x: 0, s: 0 }, map), far = worldToMinimap({ x: 0, s: 400 }, map);
    assert.ok(near.x < far.x);
    assert.equal(near.y, far.y);
    assert.ok(near.x >= 7.99);
    assert.ok(far.x <= 172.01);
    const center = worldToMinimap({ x: 0, s: 200 }, map);
    assert.equal(center.x, 90);
    assert.equal(center.y, 55);
});
test('click conversion is reversible for off-origin and small designs', () => {
    for (const bounds of [{ minX: 100, maxX: 140, minS: -900, maxS: -300 }, { minX: 0, maxX: 0, minS: 0, maxS: 0 }]) {
        const map = minimapTransform(bounds, 160, 100), point = { x: bounds.minX + 5, s: bounds.minS + 3 };
        const restored = minimapToWorld(worldToMinimap(point, map), map);
        assert.ok(Math.hypot(restored.x - point.x, restored.s - point.s) < 1e-8);
    }
});
test('temporary preview movement cannot mutate the initial camera or exported scene', () => {
    const p = template('empty'), before = JSON.stringify(p), camera = p.camera;
    const offsets = previewOffsets(camera, { x: 120, s: 900 });
    assert.equal(offsets.walkX, 120);
    assert.equal(offsets.walk, 900);
    assert.equal(JSON.stringify(p), before);
    assert.deepEqual(exportScene(p).scene.camera.position_cm, [0, 10, -0]);
});
test('elevation is independent from object size and reaches preview and exported geometry', () => {
    for (const shape of ['sphere', 'cube', 'diamond', 'cylinder']) {
        const o = object('landmark', { shape, pattern: 'solid', pattern_color: '#ffffff', pattern_size_cm: 5, w: 30, h: 40, d: 20, x: 0, s: 100 });
        const raised = landmarkAtElevation(o, 50);
        assert.equal(raised.y, 50 + landmarkHeight(raised) / 2);
        assert.equal(landmarkElevation(raised), 50);
        const p = template('empty');
        p.objects = [raised];
        const geometry = solids(p);
        const bottom = Math.min(...geometry.map(g => g.y - (g.kind === 'sphere' ? g.w : g.h) / 2));
        assert.ok(Math.abs(bottom - 50) < 1e-8);
        assert.equal(solids(p, { preview: true })[0].y, raised.y);
        const exported = Object.values(exportScene(p).scene.stimuli);
        assert.equal(exported.length, geometry.length);
        assert.ok(exported.every(entry => entry.stimulus.body.transform.position_cm[1] >= 50));
        const resized = landmarkAtElevation({ ...raised, w: 60, h: 70 }, landmarkElevation(raised));
        assert.equal(landmarkElevation(resized), 50);
    }
});
test('legacy floating spheres preserve their base elevation when resized', () => {
    const old = object('sphere', { w: 30, y: 65 });
    assert.equal(landmarkElevation(old), 50);
    const resized = landmarkAtElevation({ ...old, w: 50 }, landmarkElevation(old));
    assert.equal(resized.y, 75);
    assert.equal(landmarkElevation(resized), 50);
});
