import test from 'node:test';
import assert from 'node:assert/strict';
import { cameraMovement, constrainMovement, constrainPreview, validateMovementStart } from './collisions.mjs';
import { object, template, exportScene } from './model.mjs';
const near = (a, b) => assert.ok(Math.abs(a - b) < 1e-6, `${a} != ${b}`);
test('start boundary respects offset and heading, with no reverse debt', () => {
    for (const yaw of [0, 37, 90, 180, -90]) {
        const p = template('empty');
        p.camera = { ...p.camera, x: 100, s: 200, yaw };
        const m = cameraMovement(p), a = m.start_position_cm, r = yaw * Math.PI / 180, forward = [Math.sin(r), 0, -Math.cos(r)];
        const point = distance => a.map((v, i) => v + forward[i] * distance);
        constrainMovement(m, a, point(-10000)).forEach((v, i) => near(v, a[i]));
        constrainMovement(m, a, point(5)).forEach((v, i) => near(v, point(5)[i]));
        constrainMovement(m, point(5), point(-100)).forEach((v, i) => near(v, a[i]));
    }
});
test('swept collision blocks fast motion against rotated thin walls and every landmark shape', () => {
    for (const o of [object('wall', { s: 50, w: 100, d: .1, angle: 30 }), ...['sphere', 'cube', 'cylinder', 'diamond'].map(shape => object('landmark', { shape, pattern: 'bands', x: 0, s: 50, y: 10, w: 20, h: 20, d: 20, angle: 37 }))]) {
        const p = template('empty');
        p.objects = [o];
        const m = cameraMovement(p), start = [0, 10, 0];
        validateMovementStart(m);
        const end = constrainMovement(m, start, [0, 10, -10000]);
        assert.ok(end[2] > -50 && end[2] < -20);
        const held = constrainMovement(m, end, [0, 10, -10000]);
        near(held[2], end[2]);
        near(constrainMovement(m, held, [0, 10, held[2] + 5])[2], held[2] + 5);
        // Shading/tessellation never changes collision volumes.
        o.pattern = 'solid';
        assert.deepEqual(cameraMovement(p), m);
    }
});
test('sphere collision follows curvature, arches keep their openings and elevated objects can be passed under', () => {
    const p = template('empty');
    p.objects = [object('sphere', { x: 0, s: 50, y: 10, w: 20 }), object('floor', { s: 50, w: 100, d: 100 })];
    let m = cameraMovement(p);
    assert.equal(m.obstacles.length, 1);
    near(constrainMovement(m, [20, 10, 0], [20, 10, -100])[2], -100);
    near(constrainMovement(m, [0, 30, 0], [0, 30, -100])[2], -100);
    p.objects = [object('arch', { s: 50, w: 40, h: 40 })];
    m = cameraMovement(p);
    assert.equal(m.obstacles.length, 3);
    near(constrainMovement(m, [0, 10, 0], [0, 10, -100])[2], -100);
    assert.ok(constrainMovement(m, [22, 10, 0], [22, 10, -100])[2] > -50);
    assert.ok(constrainMovement(m, [0, 42, 0], [0, 42, -100])[2] > -50);
});
test('preview keyboard/slider/map movements use the same sweep and export refuses an obstructed initial camera', () => {
    const p = template('empty');
    p.objects = [object('wall', { s: 50, w: 100, d: 2 })];
    const moved = constrainPreview(p, { x: 0, s: 0 }, { x: 0, s: 1000 });
    assert.ok(moved.s < 48 && moved.s > 47);
    near(constrainPreview(p, moved, { x: 0, s: -100 }).s, 0);
    p.objects[0].s = 0;
    assert.throws(() => exportScene(p), /Initial camera overlaps/);
});
test('scene v6 carries independent logical obstacles, start origin and native wheel loop', () => {
    const p = template();
    p.navigation = { mode: 'wheel', device: 'wheel', axis: 'wheel', loop: true, track_length_cm: 400, fade_frames: 15 };
    const result = exportScene(p);
    assert.equal(result.version, 6);
    assert.deepEqual(result.scene.camera_movement, cameraMovement(p));
    assert.equal(result.scene.animations[1].animation.LinearNav3D.track.length_cm, 400);
    assert.equal(p.objects.filter(o => o.kind === 'floor').length, 40);
    assert.equal(result.scene.camera_movement.obstacles.length, 49);
});
