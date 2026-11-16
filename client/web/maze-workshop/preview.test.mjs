import test from 'node:test';
import assert from 'node:assert/strict';
import { DepthPreview } from './preview.mjs';
import { object, solids, template } from './model.mjs';
const camera = { x: 0, s: 0, y: 10, yaw: 0, pitch: 0, fov: 60 };
const pixel = (image, x, y) => Array.from(image.data.slice((y * image.width + x) * 4, (y * image.width + x) * 4 + 4));
function render(objects, cam = camera) { return new DepthPreview().render(objects, cam, 320, 180); }
const box = v => ({ kind: 'box', x: 0, s: 200, y: -.5, w: 40, h: 1, d: 400, angle: 0, color: '#808080', ...v });
test('a distant landmark stays visible above a continuous floor regardless of draw order', () => {
    const floor = box({}), landmark = box({ x: 0, s: 300, y: 3, w: 12, h: 6, d: 12, color: '#ff0000' });
    const a = render([landmark, floor]), b = render([floor, landmark]);
    assert.deepEqual(pixel(a, 160, 94), pixel(b, 160, 94));
    const p = pixel(a, 160, 94);
    assert.ok(p[0] > 100 && p[1] === 0 && p[2] === 0 && p[3] === 255, `landmark: ${p}`);
});
test('continuous and tiled floors give the same visibility from the start and after advancing', () => {
    const floor = box({}), tiles = Array.from({ length: 20 }, (_, i) => box({ s: i * 20 + 10, d: 20 }));
    const wall = box({ x: 22, s: 200, y: 15, w: 4, h: 30, d: 400, color: '#ee8800' });
    const marker = box({ s: 300, y: 3, w: 12, h: 6, d: 12, color: '#ff0000' });
    for (const s of [0, 50, 200]) {
        const a = render([floor, wall, marker], { ...camera, s }), b = render([...tiles, wall, marker], { ...camera, s });
        for (const [x, y] of [[160, 100], [145, 110], [180, 115], [160, 94], [200, 95]])
            assert.deepEqual(pixel(a, x, y), pixel(b, x, y), `camera ${s}, pixel ${x},${y}`);
    }
});
test('a continuous wall correctly occludes a landmark behind it, in either object order', () => {
    const wall = box({ x: 0, s: 100, y: 10, w: 100, h: 30, d: 4, color: '#008800' });
    const marker = box({ s: 300, y: 10, w: 30, h: 30, d: 10, color: '#ff0000' });
    for (const list of [[wall, marker], [marker, wall]]) {
        const p = pixel(render(list), 160, 90);
        assert.ok(p[0] === 0 && p[1] > 0 && p[2] === 0 && p[3] === 255);
    }
});
test('geometry behind the camera is clipped, geometry far away remains visible', () => {
    const behind = box({ s: -100, y: 10, w: 40, h: 40, d: 10 });
    assert.equal(render([behind]).data.some(v => v !== 0), false);
    const far = box({ s: 8000, y: 10, w: 2000, h: 2000, d: 10, color: '#ff0000' });
    assert.ok(pixel(render([far]), 160, 90)[0] > 100);
});
test('preview renders template and empty scenes and reuses buffers without retaining old pixels', () => {
    const renderer = new DepthPreview();
    const p = template();
    const image = renderer.render(solids(p), camera, 320, 180);
    assert.ok(image.data.some(v => v !== 0));
    const empty = renderer.render([], camera, 320, 180);
    assert.equal(empty.data.some(v => v !== 0), false);
});
test('continuous walls and segmented walls have the same distant visibility while navigating', () => {
    const long = [-22, 22].map(x => box({ x, s: 200, y: 15, w: 4, h: 30, d: 400, color: '#ee8800' }));
    const tiled = [-22, 22].flatMap(x => Array.from({ length: 20 }, (_, i) => box({ x, s: 10 + i * 20, y: 15, w: 4, h: 30, d: 20, color: '#ee8800' })));
    const marker = box({ x: 0, s: 350, y: 8, w: 15, h: 16, d: 5, color: '#0000ff' });
    for (const s of [0, 100, 250]) {
        const a = render([...long, marker], { ...camera, s }), b = render([...tiled, marker], { ...camera, s });
        assert.ok(a.data.every((v, i) => v === b.data[i]), `long and tiled walls, camera forward ${s}`);
    }
});
