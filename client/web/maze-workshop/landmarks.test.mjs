import test from 'node:test';
import assert from 'node:assert/strict';
import { landmarkShapes, landmarkPatterns, landmarkGeometry, landmarkFootprint } from './landmarks.mjs';
import { object, template, validate, exportScene, solids, resizeCorners, resizeFromCorner, objectsInsideRect } from './model.mjs';
import { DepthPreview } from './preview.mjs';
const landmark = (values = {}) => object('landmark', { shape: 'sphere', pattern: 'solid', pattern_color: '#ffffff', pattern_size_cm: 5, color: '#3366cc', x: 20, s: 100, y: 30, w: 30, d: 20, h: 40, ...values });
test('export gives landmarks volume cues in vstimd, including legacy spheres and wheel scenes', () => {
    for (const navigation of [{ mode: 'static' }, { mode: 'wheel', device: 'wheel', axis: 'wheel', loop: true, track_length_cm: 400, fade_frames: 15 }]) {
        for (const o of [object('sphere', { w: 30, x: 20, y: 45, s: 100 }), ...landmarkShapes.flatMap(shape => landmarkPatterns.map(pattern => landmark({ shape, pattern })))]) {
            const p = template('empty');
            p.navigation = navigation;
            p.objects = [o, object('wall'), object('floor'), object('arch')];
            const scene = exportScene(p).scene, bodies = Object.values(scene.stimuli).map(e => e.stimulus.body), count = solids({ objects: [o] }).length;
            assert.ok(bodies.slice(0, count).every(b => b.material.shading === 'Phong'), `${o.shape || o.kind}/${o.pattern || 'solid'}`);
            assert.ok(bodies.slice(count).every(b => b.material.shading === 'Unlit'));
            if (o.kind === 'sphere' || (o.shape === 'sphere' && o.pattern === 'solid')) {
                assert.equal(count, 1);
                assert.deepEqual(bodies[0].geometry, { type: 'Sphere', diameter_cm: o.w, rings: 16, sectors: 24 });
                assert.deepEqual(bodies[0].transform.position_cm, [o.x, o.y, -o.s]);
                assert.deepEqual(bodies[0].transform.scale, [1, 1, 1]);
            }
            assert.ok(scene.lighting.ambient_color.every(v => v > 0));
            assert.ok(scene.lighting.sun_color.every(v => v > 0));
            if (navigation.mode === 'wheel')
                assert.deepEqual(scene.animations[1].animation.LinearNav3D.source, { device: 'wheel', axis: 'wheel' });
        }
    }
});
test('all four shapes and patterns round trip and export exclusively supported geometry and real colours', () => {
    for (const shape of landmarkShapes)
        for (const pattern of landmarkPatterns) {
            const o = landmark({ shape, pattern, angle: 27 }), p = template('empty');
            p.objects = [o];
            const restored = validate(JSON.parse(JSON.stringify(p)));
            assert.deepEqual(restored.objects[0], o);
            const geometry = solids(restored);
            assert.ok(geometry.length > 0 && geometry.length < 4000, `${shape}/${pattern}: ${geometry.length}`);
            const colors = new Set(geometry.map(p => p.color));
            assert.ok(colors.has(o.color));
            if (pattern !== 'solid')
                assert.ok(colors.has(o.pattern_color));
            for (const part of geometry) {
                assert.ok(['box', 'sphere'].includes(part.kind));
                for (const k of ['w', 'd', 'h'])
                    assert.ok(part[k] > 0 && Number.isFinite(part[k]));
            }
            const exported = exportScene(restored);
            assert.equal(Object.keys(exported.scene.stimuli).length, geometry.length);
            assert.ok(Object.values(exported.scene.stimuli).every(s => ['Cube', 'Sphere'].includes(s.stimulus.body.geometry.type) && s.stimulus.body.texture_path === null));
        }
});
test('simple sphere/cube are native primitives; diamond/cylinder approximations retain size and position', () => {
    assert.equal(landmarkGeometry(landmark()).length, 1);
    assert.equal(landmarkGeometry(landmark({ shape: 'cube' })).length, 1);
    for (const shape of ['diamond', 'cylinder']) {
        const o = landmark({ shape, angle: 90 }), parts = landmarkGeometry(o);
        assert.equal(parts.length, 48);
        for (const p of parts) {
            assert.equal(p.angle, 90);
            assert.ok(p.y - p.h / 2 >= o.y - o.h / 2 - 1e-8);
            assert.ok(p.y + p.h / 2 <= o.y + o.h / 2 + 1e-8);
        }
    }
    const footprint = landmarkFootprint(landmark({ shape: 'cylinder', d: 2 }));
    assert.equal(footprint.d, 30);
});
test('landmark resize and marquee follow the shape footprint while preserving editable pattern', () => {
    for (const shape of landmarkShapes) {
        const o = landmark({ shape, pattern: 'diamonds' }), points = resizeCorners(o);
        const patch = resizeFromCorner(o, 2, { x: points[2][0] + 10, s: points[2][1] + 10 }, 1);
        assert.ok(patch.w > o.w);
        if (shape === 'sphere' || shape === 'cylinder')
            assert.equal(patch.w, patch.d);
        const result = { ...o, ...patch };
        assert.equal(result.pattern, 'diamonds');
        assert.deepEqual(objectsInsideRect([o], { x: -100, s: 0 }, { x: 200, s: 200 }), [o.id]);
    }
});
test('unknown shapes/patterns, bad colours and unsafe dimensions are rejected; legacy sphere still loads', () => {
    for (const patch of [{ shape: 'mesh' }, { pattern: 'image' }, { pattern_color: 'red' }, { pattern_size_cm: 0 }, { pattern_tilt_deg: 181 }]) {
        const p = template('empty');
        p.objects = [landmark(patch)];
        assert.throws(() => validate(p));
    }
    assert.doesNotThrow(() => validate(template()));
});
test('patterned landmark render changes colour layout between patterns and clears safely', () => {
    const renderer = new DepthPreview(), camera = { x: -40, s: 0, y: 50, yaw: Math.atan2(60, 100) * 180 / Math.PI, pitch: .1, fov: 50 };
    const signatures = [];
    for (const pattern of ['diamonds', 'spiral', 'checker']) {
        const image = renderer.render(solids({ objects: [landmark({ shape: 'sphere', pattern })] }, { preview: true }), camera, 160, 120);
        assert.ok(image.data.some(v => v !== 0));
        signatures.push(Buffer.from(image.data).toString('base64'));
    }
    assert.equal(new Set(signatures).size, 3);
});
