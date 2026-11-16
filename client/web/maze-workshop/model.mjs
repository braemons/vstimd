import { cameraMovement, validateMovementStart } from './collisions.mjs';
import { validateNavigation, wheelAnimation } from './wheel.mjs';
import { landmarkShapes, landmarkPatterns, isLandmark, isRoundLandmark, landmarkFootprint, landmarkGeometry } from './landmarks.mjs';
// Coordinates in cm: x right, s forward. vstimd uses z = -s.
export const clone = value => JSON.parse(JSON.stringify(value));
export const backgroundColor = project => project.background_color ?? '#111617';
export const uid = () => globalThis.crypto.randomUUID();
export function object(kind, values = {}) {
    return { id: uid(), kind, name: ({ wall: 'Wall', floor: 'Floor', sphere: 'Landmark', landmark: 'Landmark', arch: 'Arch' })[kind], x: 0, s: 100, y: 0, w: 40, d: 4, h: 30, angle: 0, color: '#ed8b43', ...values };
}
export function template(type = 'corridor') {
    const items = [];
    const add = (kind, v) => items.push(object(kind, v));
    if (type === 'corridor') {
        for (let r = 0; r < 20; r++)
            for (let c = 0; c < 2; c++)
                add('floor', { name: `Tile ${r + 1} · ${c + 1}`, x: c * 20 - 10, s: r * 20 + 10, w: 20, d: 20, h: 1, color: (r + c) % 2 ? '#353b3c' : '#aab2ae' });
        for (const x of [-22, 22]) {
            add('wall', { x, s: 200, w: 4, d: 400, h: 25, color: '#252b2c', name: x < 0 ? 'Left wall' : 'Right wall' });
            for (let r = 0; r < 20; r++)
                add('wall', { x: x < 0 ? -19.8 : 19.8, s: 5 + r * 20, w: .3, d: 10, h: 25, color: '#ed8b43', name: 'Orange stripe' });
        }
        add('arch', { s: 0, w: 40, d: 4, h: 40, color: '#b5bfba', name: 'Entrance arch' });
        add('arch', { s: 400, w: 40, d: 4, h: 40, color: '#b5bfba', name: 'Exit arch' });
        add('sphere', { x: 60, s: 550, y: 65, w: 30, d: 30, h: 30, color: '#437aff', name: 'Blue landmark' });
    }
    else if (type === 'tee') {
        add('floor', { s: 125, w: 40, d: 250, color: '#aab2ae' });
        add('floor', { s: 270, w: 240, d: 40, color: '#aab2ae' });
        for (const x of [-22, 22])
            add('wall', { x, s: 125, w: 4, d: 250, h: 30, color: '#ed8b43' });
        add('wall', { s: 292, w: 248, d: 4, h: 30, color: '#ed8b43' });
        for (const x of [-82, 82])
            add('wall', { x, s: 248, w: 124, d: 4, h: 30, color: '#ed8b43' });
        add('sphere', { x: -100, s: 270, y: 45, w: 15, d: 15, h: 15, color: '#437aff', name: 'Left landmark' });
        add('sphere', { x: 100, s: 270, y: 45, w: 15, d: 15, h: 15, color: '#f1c95d', name: 'Right landmark' });
    }
    return { format: 'braemons-maze-editor', version: 1, name: type === 'tee' ? 'T-maze' : type === 'empty' ? 'My maze' : 'Maze project', background_color: '#111617', camera: { x: 0, s: 0, y: 10, yaw: 0, fov: 60 }, objects: items };
}
export function validate(p) {
    const fail = m => { throw new Error(m); };
    if (!p || p.format !== 'braemons-maze-editor' || p.version !== 1)
        fail('Choose an editor .maze.json project (version 1).');
    if (typeof p.name !== 'string' || p.name.length > 120)
        fail('Invalid project name.');
    if (p.background_color !== undefined && (typeof p.background_color !== 'string' || !/^#[0-9a-f]{6}$/i.test(p.background_color)))
        fail('Invalid environment background colour.');
    const num = (v, min, max, label) => { if (!Number.isFinite(v) || v < min || v > max)
        fail(`${label}: value outside the range ${min}–${max}.`); };
    if (!p.camera)
        fail('Camera is missing.');
    for (const k of ['x', 's', 'y', 'yaw', 'fov'])
        num(p.camera[k], k === 'fov' ? 20 : k === 'y' ? 1 : -10000, k === 'fov' ? 120 : 10000, `Camera ${k}`);
    if (!Array.isArray(p.objects) || p.objects.length > 1000)
        fail('Projects support up to 1000 objects.');
    if (p.navigation !== undefined)
        validateNavigation(p.navigation);
    const ids = new Set();
    for (const o of p.objects) {
        if (!o || !['wall', 'floor', 'sphere', 'arch', 'landmark'].includes(o.kind) || typeof o.id !== 'string' || !o.id || ids.has(o.id))
            fail('Unknown object or duplicate identifier.');
        ids.add(o.id);
        if (typeof o.name !== 'string' || o.name.length > 120 || !/^#[0-9a-f]{6}$/i.test(o.color))
            fail('Invalid object name or colour.');
        if (o.kind === 'landmark') {
            if (!landmarkShapes.includes(o.shape) || !landmarkPatterns.includes(o.pattern) || !/^#[0-9a-f]{6}$/i.test(o.pattern_color))
                fail('Invalid landmark shape, pattern or colour.');
            num(o.pattern_size_cm, .1, 10000, 'Pattern size · cm');
            if (o.pattern_tilt_deg !== undefined)
                num(o.pattern_tilt_deg, -180, 180, 'Pattern tilt · degrees');
        }
        for (const k of ['x', 's', 'y', 'angle'])
            num(o[k], -10000, 10000, k);
        for (const k of ['w', 'd', 'h'])
            num(o[k], .1, 10000, k);
    }
    return clone(p);
}
export function solids(p, { preview = false } = {}) {
    const result = [];
    for (const o of p.objects) {
        const push = (dx, dy, w, h, d) => {
            const a = o.angle * Math.PI / 180;
            result.push({ ...o, x: o.x + dx * Math.cos(a), s: o.s + dx * Math.sin(a), y: dy, w, h, d, kind: 'box' });
        };
        if (preview && isLandmark(o))
            result.push({ ...o, kind: 'analytic-landmark', shape: o.shape || 'sphere' });
        else if (o.kind === 'landmark')
            result.push(...landmarkGeometry(o));
        else if (o.kind === 'sphere')
            result.push({ ...o, kind: 'sphere' });
        else if (o.kind === 'arch') {
            push(-o.w / 2 - 2, o.y + o.h / 2, 4, o.h, o.d);
            push(o.w / 2 + 2, o.y + o.h / 2, 4, o.h, o.d);
            push(0, o.y + o.h + 2, o.w + 8, 4, o.d);
        }
        else
            push(0, o.kind === 'floor' ? o.y - .5 : o.y + o.h / 2, o.w, o.kind === 'floor' ? 1 : o.h, o.d);
    }
    return result;
}
export function exportScene(project) {
    const p = validate(project), stimuli = {};
    const movement = cameraMovement(p);
    validateMovementStart(movement);
    const geometry = solids(p);
    if (geometry.length > 50000)
        throw new Error('This scene is too detailed to export. Reduce the number of patterned landmarks.');
    geometry.forEach((o, i) => {
        const rgb = o.color.match(/\w\w/g).map(v => parseInt(v, 16) / 255);
        // Preserve volume cues, as in vstimd's corridor example. Pattern fragments
        // carry preview_landmark after their kind has become box/sphere.
        const shading = isLandmark(o) || o.preview_landmark ? 'Phong' : 'Unlit';
        stimuli[i + 1] = { id: uid(), name: o.name, stimulus: { common: { flags: { enabled: true, protected: false }, opacity: 1 }, body: { type: 'Mesh3d', transform: { position_cm: [o.x, o.y, -o.s], rotation_deg: [o.angle, 0, 0], scale: [1, 1, 1] }, material: { albedo: [...rgb, 1], emissive: [0, 0, 0], shading }, geometry: o.kind === 'sphere' ? { type: 'Sphere', diameter_cm: o.w, rings: 16, sectors: 24 } : { type: 'Cube', size_cm: [o.w, o.h, o.d] }, texture_path: null } } };
    });
    const navigation = wheelAnimation(p.navigation);
    return { version: 6, scene: { camera_movement: movement, background: [...backgroundColor(p).match(/\w\w/g).map(v => parseInt(v, 16) / 255), 1], default_fill: [1, 1, 1, 1], default_outline: [0, 0, 0, 1], photodiode: { enabled: false, lit: false, flicker: false, position: 0 }, stimuli, next_stim_handle: Object.keys(stimuli).length + 1, animations: navigation ? { 1: navigation } : {}, next_anim_handle: navigation ? 2 : 1, camera: { position_cm: [p.camera.x, p.camera.y, -p.camera.s], yaw_deg: -p.camera.yaw, pitch_deg: 0, roll_deg: 0, fov_y_deg: p.camera.fov, near_cm: .1, far_cm: 30000 }, lighting: { ambient_color: [.4, .4, .4], sun_direction: [-.5, -1, -.3], sun_color: [.6, .6, .6] }, camera_zones: [] }, io: { vtl: { names: [] } } };
}
export function corners(o) {
    if (isLandmark(o))
        o = landmarkFootprint(o);
    const a = o.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
    return [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([u, v]) => [o.x + c * u * o.w / 2 - s * v * o.d / 2, o.s + s * u * o.w / 2 + c * v * o.d / 2]);
}
export function hit(o, x, s) {
    if (isRoundLandmark(o))
        return Math.hypot(x - o.x, s - o.s) <= Math.max(o.w / 2, 3);
    if (isLandmark(o))
        o = landmarkFootprint(o);
    const a = -o.angle * Math.PI / 180, dx = x - o.x, ds = s - o.s;
    return Math.abs(Math.cos(a) * dx - Math.sin(a) * ds) <= Math.max(o.w / 2, 3) && Math.abs(Math.sin(a) * dx + Math.cos(a) * ds) <= Math.max(o.d / 2, 3);
}
// Equal clear gaps along the row joining the furthest selected centres.
// Keep the two outer objects fixed and preserve perpendicular offsets.
export function distributeEvenly(objects) {
    if (objects.length < 3)
        throw new Error('Select at least 3 objects to distribute.');
    let first = objects[0], last = objects[1], distance2 = 0;
    for (let i = 0; i < objects.length; i++)
        for (let j = i + 1; j < objects.length; j++) {
            const d = (objects[j].x - objects[i].x) ** 2 + (objects[j].s - objects[i].s) ** 2;
            if (d > distance2) {
                distance2 = d;
                first = objects[i];
                last = objects[j];
            }
        }
    if (distance2 < 1e-12)
        throw new Error('Move the outer objects apart before distributing.');
    const length = Math.sqrt(distance2);
    let ux = (last.x - first.x) / length, us = (last.s - first.s) / length;
    if (ux < 0 || (Math.abs(ux) < 1e-12 && us < 0)) {
        ux = -ux;
        us = -us;
    }
    const extent = o => {
        if (isRoundLandmark(o))
            return o.w;
        const a = o.angle * Math.PI / 180, w = o.w + (o.kind === 'arch' ? 8 : 0);
        return Math.abs(ux * Math.cos(a) + us * Math.sin(a)) * w + Math.abs(-ux * Math.sin(a) + us * Math.cos(a)) * o.d;
    };
    const row = objects.map(o => ({ o, t: o.x * ux + o.s * us, width: extent(o) })).sort((a, b) => a.t - b.t || a.o.id.localeCompare(b.o.id));
    const start = row[0], end = row.at(-1);
    const gapCm = (end.t - start.t - start.width / 2 - end.width / 2 - row.slice(1, -1).reduce((sum, r) => sum + r.width, 0)) / (row.length - 1);
    if (gapCm < -1e-8)
        throw new Error('Not enough room. Move the outer objects further apart.');
    const gap = Math.max(0, gapCm), positions = [{ id: start.o.id, x: start.o.x, s: start.o.s }];
    let t = start.t;
    for (let i = 1; i < row.length - 1; i++) {
        t += row[i - 1].width / 2 + gap + row[i].width / 2;
        const shift = t - row[i].t;
        positions.push({ id: row[i].o.id, x: row[i].o.x + ux * shift, s: row[i].o.s + us * shift });
    }
    positions.push({ id: end.o.id, x: end.o.x, s: end.o.s });
    return { positions, gapCm: gap };
}
// Decorative strips on the long faces of a wall; ordinary editable wall boxes.
export function wallPanels(wall, { width_cm = 10, gap_cm = 10, color = '#ed8b43', sides = 'both' } = {}, maxPanels = 1000) {
    if (!wall || wall.kind !== 'wall')
        throw new Error('Select a single wall to add panels.');
    for (const [name, value] of [['Panel width', width_cm], ['Gap', gap_cm]])
        if (!Number.isFinite(value) || value < .1 || value > 10000)
            throw new Error(`${name} must be between 0.1 and 10000 cm.`);
    if (!/^#[0-9a-f]{6}$/i.test(color) || !['both', 'a', 'b'].includes(sides))
        throw new Error('Invalid panel colour or side.');
    const alongWidth = wall.w >= wall.d, length = alongWidth ? wall.w : wall.d;
    const signs = sides === 'both' ? [-1, 1] : [sides === 'a' ? -1 : 1], period = width_cm + gap_cm;
    let count = Math.ceil(length / period);
    if (count && length - (count - 1) * period < .1)
        count--;
    if (count * signs.length > maxPanels)
        throw new Error('Too many panels for this project. Increase width or gap, or choose one side.');
    const angle = wall.angle * Math.PI / 180, c = Math.cos(angle), s = Math.sin(angle), panels = [];
    for (const sign of signs)
        for (let i = 0; i < count; i++) {
            const width = Math.min(width_cm, length - i * period), along = -length / 2 + i * period + width / 2;
            const normal = sign * ((alongWidth ? wall.d : wall.w) / 2 + .1 + .02);
            const u = alongWidth ? along : normal, v = alongWidth ? normal : along;
            panels.push(object('wall', { name: `${wall.name.slice(0, 85)} · panel ${i + 1}${sign < 0 ? ' A' : ' B'}`, x: wall.x + c * u - s * v, s: wall.s + s * u + c * v, y: wall.y, w: alongWidth ? width : .2, d: alongWidth ? .2 : width, h: wall.h, angle: wall.angle, color }));
        }
    return panels;
}
export function resizeCorners(o) { return corners(isLandmark(o) ? landmarkFootprint(o) : o); }
export function resizeFromCorner(original, index, pointer, grid_cm = 1) {
    if (!Number.isInteger(index) || index < 0 || index > 3 || !Number.isFinite(pointer.x) || !Number.isFinite(pointer.s) || !Number.isFinite(grid_cm) || grid_cm <= 0)
        throw new Error('Invalid resize position.');
    const signs = [[-1, -1], [1, -1], [1, 1], [-1, 1]], [sx, ss] = signs[index];
    const anchor = resizeCorners(original)[(index + 2) % 4];
    const angle = ((original.kind === 'sphere' || original.shape === 'sphere') ? 0 : original.angle) * Math.PI / 180, c = Math.cos(angle), s = Math.sin(angle);
    const dx = pointer.x - anchor[0], ds = pointer.s - anchor[1];
    const quantize = value => Math.max(.1, Math.min(10000, Math.round(value / grid_cm) * grid_cm));
    let w = quantize(sx * (c * dx + s * ds)), d = quantize(ss * (-s * dx + c * ds));
    if (isRoundLandmark(original))
        w = d = Math.max(w, d);
    const u = sx * w / 2, v = ss * d / 2;
    return { x: anchor[0] + c * u - s * v, s: anchor[1] + s * u + c * v, w, d };
}
export function objectsInsideRect(objects, start, end) {
    const x0 = Math.min(start.x, end.x), x1 = Math.max(start.x, end.x), s0 = Math.min(start.s, end.s), s1 = Math.max(start.s, end.s), eps = 1e-8;
    return objects.filter(o => {
        const shape = isLandmark(o) ? landmarkFootprint(o) : o.kind === 'arch' ? { ...o, w: o.w + 8 } : o;
        return corners(shape).every(([x, s]) => x >= x0 - eps && x <= x1 + eps && s >= s0 - eps && s <= s1 + eps);
    }).map(o => o.id);
}
