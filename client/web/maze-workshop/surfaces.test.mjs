import test from 'node:test';
import assert from 'node:assert/strict';
import { intersectSurface, surfacePattern } from './surfaces.mjs';
import { object, solids } from './model.mjs';
import { DepthPreview } from './preview.mjs';
const dimensions = { w: 30, h: 40, d: 20 };
const cue = (values = {}) => object('landmark', { shape: 'sphere', pattern: 'bands', pattern_color: '#ffffff', pattern_size_cm: 7, pattern_tilt_deg: 35, color: '#121212', x: 0, s: 100, y: 20, w: 30, h: 40, d: 20, ...values });
const camera = { x: 0, s: 0, y: 20, yaw: 0, pitch: 0, fov: 40 };
const pixel = (image, x, y) => Array.from(image.data.slice((y * image.width + x) * 4, (y * image.width + x) * 4 + 4));
test('sphere intersections are analytic and preserve smooth normals', () => {
    const hit = intersectSurface('sphere', { w: 30, h: 30, d: 30 }, [0, 0, -100], [0, 0, 1]);
    assert.equal(hit.t, 85);
    assert.deepEqual(hit.point, [0, 0, -15]);
    assert.deepEqual(hit.normal, [0, 0, -1]);
    assert.equal(intersectSurface('sphere', dimensions, [16, 0, -100], [0, 0, 1]), null);
    assert.equal(intersectSurface('sphere', dimensions, [0, 0, 0], [0, 0, 1]).t, 15);
});
test('cylinder includes curved side and flat caps; cube and diamond preserve their geometry', () => {
    assert.equal(intersectSurface('cylinder', dimensions, [0, 0, -100], [0, 0, 1]).t, 85);
    const cap = intersectSurface('cylinder', dimensions, [0, 100, 0], [0, -1, 0]);
    assert.equal(cap.t, 80);
    assert.deepEqual(cap.normal, [0, 1, 0]);
    assert.equal(intersectSurface('cube', dimensions, [0, 0, -100], [0, 0, 1]).t, 90);
    assert.equal(intersectSurface('diamond', dimensions, [0, 0, -100], [0, 0, 1]).t, 90);
    assert.equal(intersectSurface('diamond', dimensions, [14, 19, -100], [0, 0, 1]), null);
});
test('wrapped spirals and checker patterns have no longitudinal seam; tilt changes the pattern', () => {
    for (const pattern of ['spiral', 'checker', 'diamonds']) {
        const o = cue({ pattern, pattern_tilt_deg: 0 });
        assert.equal(surfacePattern(o, [1e-8, 3, 14]), surfacePattern(o, [-1e-8, 3, 14]), pattern);
    }
    let different = 0;
    for (let y = -12; y <= 12; y += 2)
        for (let x = -10; x <= 10; x += 2) {
            const z = -Math.sqrt(Math.max(0, 225 - x * x - y * y));
            different += surfacePattern(cue({ pattern_tilt_deg: 0 }), [x, y, z]) !== surfacePattern(cue(), [x, y, z]);
        }
    assert.ok(different > 20);
});
test('preview uses one analytic object; export keeps its explicitly approximate geometry', () => {
    const p = { objects: [cue()] };
    assert.equal(solids(p, { preview: true }).length, 1);
    assert.equal(solids(p, { preview: true })[0].kind, 'analytic-landmark');
    assert.ok(solids(p).length > 100);
});
test('subpixel sampling smooths the silhouette and procedural bands have intermediate pixel colours', () => {
    const renderer = new DepthPreview(), image = renderer.render(solids({ objects: [cue()] }, { preview: true }), camera, 240, 180);
    const alpha = [];
    for (let i = 3; i < image.data.length; i += 4)
        alpha.push(image.data[i]);
    assert.ok(alpha.some(a => a > 0 && a < 255));
    assert.ok(alpha.includes(255));
    const visible = [];
    for (let i = 0; i < image.data.length; i += 4)
        if (image.data[i + 3] === 255)
            visible.push(image.data[i]);
    assert.ok(Math.max(...visible) > 200);
    assert.ok(Math.min(...visible) < 30);
});
test('analytic landmarks stay behind real walls and overlap correctly in either object order', () => {
    const renderer = new DepthPreview(), wall = object('wall', { x: 0, s: 60, y: 0, w: 100, d: 4, h: 60, color: '#009900' });
    const hidden = renderer.render(solids({ objects: [cue(), wall] }, { preview: true }), camera, 240, 180);
    const p = pixel(hidden, 120, 90);
    assert.ok(p[0] === 0 && p[1] > 0 && p[2] === 0);
    const a = cue({ s: 80, x: -4, pattern: 'solid', color: '#ff0000' }), b = cue({ s: 110, x: 4, pattern: 'solid', color: '#0000ff' });
    const first = Buffer.from(renderer.render(solids({ objects: [a, b] }, { preview: true }), camera, 160, 120).data);
    const second = Buffer.from(renderer.render(solids({ objects: [b, a] }, { preview: true }), camera, 160, 120).data);
    assert.ok(first.equals(second));
});
