import test from 'node:test';
import assert from 'node:assert/strict';
import { template, validate, exportScene, object, solids, corners, hit, distributeEvenly, wallPanels, resizeCorners, resizeFromCorner, objectsInsideRect, backgroundColor } from './model.mjs';
test('projects round trip and exported corridor uses cm, negative Z, no runtime experiment logic', () => {
    const p = validate(JSON.parse(JSON.stringify(template()))), c = exportScene(p);
    assert.equal(c.version, 6);
    assert.equal(c.scene.camera.position_cm[1], 10);
    assert.equal(c.scene.camera_zones.length, 0);
    assert.deepEqual(c.scene.animations, {});
    assert.deepEqual(c.io.vtl.names, []);
    const objects = Object.values(c.scene.stimuli);
    assert.equal(objects.length, 89);
    assert.deepEqual(objects.find(o => o.name === 'Blue landmark').stimulus.body.transform.position_cm, [60, 65, -550]);
    assert.equal(new Set(objects.map(o => o.id)).size, 89);
    assert.ok(objects.every(o => o.stimulus.body.texture_path === null));
});
test('rotated wall preserves its planar footprint and uses vstimd yaw slot', () => {
    const p = template('empty');
    p.objects.push(object('wall', { x: 10, s: 20, w: 100, d: 4, angle: 90 }));
    const b = exportScene(p).scene.stimuli[1].stimulus.body;
    assert.deepEqual(b.transform.rotation_deg, [90, 0, 0]);
    const vertices = corners(p.objects[0]);
    assert.ok(Math.abs(Math.max(...vertices.map(p => p[1])) - 70) < 1e-8);
    assert.ok(hit(p.objects[0], 10, 65));
    assert.ok(!hit(p.objects[0], 60, 20));
});
test('arch exports three pieces with an unobstructed opening and rotates their offsets', () => {
    const p = template('empty');
    p.objects.push(object('arch', { x: 50, s: 100, y: 5, w: 40, h: 30, angle: 90 }));
    const parts = solids(p);
    assert.equal(parts.length, 3);
    assert.ok(Math.abs(parts[0].x - 50) < 1e-8);
    assert.equal(parts[0].s, 78);
    assert.equal(parts[1].s, 122);
    assert.equal(parts[2].y, 37);
    assert.equal(parts[2].w, 48);
});
test('editor positive heading points right, native camera uses negative yaw', () => {
    const p = template('empty');
    p.camera.yaw = 90;
    assert.equal(exportScene(p).scene.camera.yaw_deg, -90);
});
test('malformed and oversized imports are rejected before replacing the project', () => {
    for (const mutate of [p => p.version = 99, p => p.objects[0].w = -1, p => p.camera.y = NaN, p => p.objects[0].color = 'url(x)', p => p.objects.push({ ...p.objects[0] }), p => p.objects[0].kind = 'Mesh3d', p => p.objects = Array(1001).fill(p.objects[0])]) {
        const p = template();
        mutate(p);
        assert.throws(() => validate(p));
    }
    assert.throws(() => validate({ version: 5, scene: {} }));
});
test('distribution equalizes free gaps for unequal panel sizes, pins endpoints and preserves input', () => {
    const panels = [0, 24, 71, 120].map((x, i) => object('wall', { x, s: 20, w: [10, 20, 10, 30][i] }));
    const before = JSON.stringify(panels), result = distributeEvenly([panels[2], panels[0], panels[3], panels[1]]);
    const row = panels.map(o => ({ ...o, ...result.positions.find(p => p.id === o.id) }));
    assert.equal(JSON.stringify(panels), before);
    assert.deepEqual(result.positions.find(p => p.id === panels[0].id), { id: panels[0].id, x: 0, s: 20 });
    assert.deepEqual(result.positions.find(p => p.id === panels[3].id), { id: panels[3].id, x: 120, s: 20 });
    for (let i = 1; i < row.length; i++)
        assert.ok(Math.abs(row[i].x - row[i].w / 2 - (row[i - 1].x + row[i - 1].w / 2) - result.gapCm) < 1e-8);
});
test('distribution follows a diagonal wall and preserves perpendicular placement, height and orientation', () => {
    const c = Math.SQRT1_2;
    const panels = [0, 20, 65, 100].map((t, i) => { const perpendicular = [0, 2, -3, 0][i]; return object('wall', { x: (t - perpendicular) * c, s: (t + perpendicular) * c, w: 10, d: 4, angle: 45, y: 12 }); });
    const { positions, gapCm } = distributeEvenly(panels);
    assert.ok(Math.abs(gapCm - (100 - 30) / 3) < 1e-8);
    const row = panels.map(o => ({ ...o, ...positions.find(p => p.id === o.id) }));
    for (let i = 0; i < row.length; i++) {
        assert.ok(Math.abs((-row[i].x + row[i].s) * c - [0, 2, -3, 0][i]) < 1e-8);
        assert.equal(row[i].angle, 45);
        assert.equal(row[i].y, 12);
        if (i)
            assert.ok(Math.abs((row[i].x + row[i].s) * c - (row[i - 1].x + row[i - 1].s) * c - 10 - gapCm) < 1e-8);
    }
});
test('distribution along forward axis uses depth, supports touching panels, sphere diameter and arch outside width', () => {
    const row = [0, 15, 100].map(s => object('wall', { s, w: 90, d: 10 }));
    const result = distributeEvenly(row);
    assert.equal(result.positions[1].s, 50);
    assert.equal(result.gapCm, 40);
    const touching = [0, 10, 20].map(x => object('wall', { x, s: 0, w: 10 }));
    assert.equal(distributeEvenly(touching).gapCm, 0);
    const mixed = [object('sphere', { x: 0, s: 0, w: 10 }), object('arch', { x: 30, s: 0, w: 12 }), object('wall', { x: 100, s: 0, w: 10 })];
    assert.equal(distributeEvenly(mixed).gapCm, 35);
});
test('distribution refuses too few objects, coincident centres or insufficient room', () => {
    assert.throws(() => distributeEvenly([]), /at least 3/);
    assert.throws(() => distributeEvenly([object('wall'), object('wall'), object('wall')]), /apart/);
    assert.throws(() => distributeEvenly([0, 5, 10].map(x => object('wall', { x, w: 40 }))), /Not enough room/);
});
test('Add panels covers a long wall with alternating strips and clear gaps on both faces', () => {
    const wall = object('wall', { x: 22, s: 200, w: 4, d: 400, h: 25, y: 2 });
    const panels = wallPanels(wall);
    assert.equal(panels.length, 40);
    for (const side of [-1, 1]) {
        const row = panels.filter(o => Math.sign(o.x - wall.x) === side).sort((a, b) => a.s - b.s);
        assert.equal(row.length, 20);
        assert.equal(row[0].s, 5);
        assert.equal(row.at(-1).s, 385);
        for (let i = 0; i < row.length; i++) {
            const o = row[i];
            assert.equal(o.w, .2);
            assert.equal(o.d, 10);
            assert.equal(o.h, 25);
            assert.equal(o.y, 2);
            assert.ok(Math.abs(Math.abs(o.x - wall.x) - 2.12) < 1e-10);
            if (i)
                assert.equal(o.s - row[i - 1].s - o.d, 10);
        }
    }
    assert.equal(new Set(panels.map(o => o.id)).size, 40);
});
test('Add panels follows a rotated wall, trims the end strip, and supports a single face', () => {
    const wall = object('wall', { x: 100, s: 50, w: 55, d: 4, h: 30, angle: 90 });
    const panels = wallPanels(wall, { width_cm: 10, gap_cm: 10, color: '#112233', sides: 'a' });
    assert.equal(panels.length, 3);
    assert.deepEqual(panels.map(o => o.w), [10, 10, 10]);
    for (const o of panels) {
        assert.ok(Math.abs(o.x - 102.12) < 1e-8);
        assert.equal(o.angle, 90);
        assert.equal(o.color, '#112233');
    }
    const partial = wallPanels({ ...wall, w: 45 }, { width_cm: 10, gap_cm: 10, sides: 'b' });
    assert.deepEqual(partial.map(o => o.w), [10, 10, 5]);
    assert.ok(Math.abs(partial.at(-1).s + partial.at(-1).w / 2 - (wall.s + 45 / 2)) < 1e-8);
    const p = template('empty');
    p.objects = [wall, ...panels];
    assert.doesNotThrow(() => exportScene(p));
});
test('Add panels validates gaps, wall type and total object limit before generating', () => {
    const wall = object('wall', { w: 400, d: 4 });
    assert.throws(() => wallPanels(wall, { gap_cm: 0 }), /Gap/);
    assert.throws(() => wallPanels(object('sphere')), /single wall/);
    assert.throws(() => wallPanels(wall, {}, 39), /Too many panels/);
    assert.equal(wallPanels({ ...wall, w: 20.05 }, { width_cm: 10, gap_cm: 10, sides: 'a' }).length, 1);
});
test('corner resize narrows and shortens a floor while fixing its opposite corner', () => {
    const floor = object('floor', { x: 0, s: 100, w: 40, d: 200, y: 2, color: '#123456' }), anchor = resizeCorners(floor)[0];
    const result = { ...floor, ...resizeFromCorner(floor, 2, { x: 10, s: 150 }, 10) };
    assert.equal(result.w, 30);
    assert.equal(result.d, 150);
    assert.equal(result.x, -5);
    assert.equal(result.s, 75);
    assert.deepEqual(resizeCorners(result)[0], anchor);
    assert.equal(result.y, 2);
    assert.equal(result.color, '#123456');
});
test('all four corners resize rotated shapes around their fixed opposite corner', () => {
    const o = object('wall', { x: 35, s: 90, w: 60, d: 20, angle: 37, h: 40 });
    for (let i = 0; i < 4; i++) {
        const anchor = resizeCorners(o)[(i + 2) % 4], point = resizeCorners(o)[i];
        const result = { ...o, ...resizeFromCorner(o, i, { x: anchor[0] + (point[0] - anchor[0]) * 2, s: anchor[1] + (point[1] - anchor[1]) * 2 }, 1) };
        assert.equal(result.w, 120);
        assert.equal(result.d, 40);
        assert.equal(result.angle, 37);
        assert.equal(result.h, 40);
        const fixed = resizeCorners(result)[(i + 2) % 4];
        assert.ok(Math.hypot(fixed[0] - anchor[0], fixed[1] - anchor[1]) < 1e-8);
    }
});
test('corner resize enforces positive size, caps dimensions and preserves spherical diameter', () => {
    const o = object('floor', { w: 40, d: 40, x: 0, s: 0 });
    const small = resizeFromCorner(o, 2, { x: -100, s: -100 }, 10);
    assert.equal(small.w, .1);
    assert.equal(small.d, .1);
    const large = resizeFromCorner(o, 2, { x: 1e6, s: 1e6 }, 10);
    assert.equal(large.w, 10000);
    assert.equal(large.d, 10000);
    const sphere = object('sphere', { w: 20, d: 4, x: 0, s: 0, angle: 40 });
    const patch = resizeFromCorner(sphere, 2, { x: 30, s: 40 }, 10);
    assert.equal(patch.w, 50);
    assert.equal(patch.d, 50);
    const resized = { ...sphere, ...patch };
    assert.deepEqual(resizeCorners(resized)[0], resizeCorners(sphere)[0]);
    assert.throws(() => resizeFromCorner(o, 5, { x: 0, s: 0 }), /Invalid/);
});
test('box selection includes enclosed panels without selecting their long wall or underlying floor', () => {
    const floor = object('floor', { w: 100, d: 400, s: 200 }), wall = object('wall', { w: 4, d: 400, x: 22, s: 200 });
    const panels = [50, 70, 90].map(s => object('wall', { w: .2, d: 10, x: 19.8, s }));
    const selected = objectsInsideRect([floor, wall, ...panels], { x: 18, s: 40 }, { x: 21, s: 100 });
    assert.deepEqual(selected, panels.map(o => o.id));
});
test('box selection works in both drag directions and requires rotated objects to be entirely inside', () => {
    const o = object('wall', { w: 40, d: 4, x: 0, s: 0, angle: 45 });
    assert.deepEqual(objectsInsideRect([o], { x: -20, s: -20 }, { x: 20, s: 20 }), [o.id]);
    assert.deepEqual(objectsInsideRect([o], { x: 20, s: 20 }, { x: -20, s: -20 }), [o.id]);
    assert.deepEqual(objectsInsideRect([o], { x: -5, s: -5 }, { x: 5, s: 5 }), []);
});
test('box selection uses the sphere diameter and includes outer arch pillars', () => {
    const sphere = object('sphere', { w: 20, d: 1, x: 0, s: 0 });
    assert.deepEqual(objectsInsideRect([sphere], { x: -10, s: -5 }, { x: 10, s: 5 }), []);
    assert.deepEqual(objectsInsideRect([sphere], { x: -10, s: -10 }, { x: 10, s: 10 }), [sphere.id]);
    const arch = object('arch', { w: 40, d: 4, s: 0 });
    assert.deepEqual(objectsInsideRect([arch], { x: -20, s: -2 }, { x: 20, s: 2 }), []);
    assert.deepEqual(objectsInsideRect([arch], { x: -24, s: -2 }, { x: 24, s: 2 }), [arch.id]);
});
test('environment background round trips and exports the selected RGB, with a fallback for old projects', () => {
    const p = template('empty');
    p.background_color = '#48a2F0';
    const loaded = validate(JSON.parse(JSON.stringify(p)));
    assert.equal(backgroundColor(loaded), '#48a2F0');
    assert.deepEqual(exportScene(loaded).scene.background, [72 / 255, 162 / 255, 240 / 255, 1]);
    delete p.background_color;
    assert.equal(backgroundColor(validate(p)), '#111617');
    assert.deepEqual(exportScene(p).scene.background, [17 / 255, 22 / 255, 23 / 255, 1]);
    for (const color of [null, 123, 'red', '#fff', 'url(x)', '#GG0000'])
        assert.throws(() => validate({ ...p, background_color: color }));
});
