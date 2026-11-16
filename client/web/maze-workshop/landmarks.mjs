import { surfacePattern } from './surfaces.mjs';
// All generated surfaces are ordinary cubes/spheres supported by vstimd v5.
// Curved patterned surfaces are a bounded geometric approximation, not textures.
export const landmarkShapes = ['sphere', 'cube', 'diamond', 'cylinder'];
export const landmarkPatterns = ['solid', 'diamonds', 'spiral', 'checker', 'bands'];
export const isLandmark = o => o.kind === 'sphere' || o.kind === 'landmark';
export const isRoundLandmark = o => o.kind === 'sphere' || (o.kind === 'landmark' && ['sphere', 'cylinder'].includes(o.shape));
export function landmarkFootprint(o) {
    if (!isLandmark(o))
        return o;
    return { ...o, d: isRoundLandmark(o) ? o.w : o.d, angle: (o.kind === 'sphere' || o.shape === 'sphere') ? 0 : o.angle };
}
export function landmarkHeight(o) { return o.kind === 'sphere' || o.shape === 'sphere' ? o.w : o.h; }
export function landmarkElevation(o) { return o.y - landmarkHeight(o) / 2; }
export function landmarkAtElevation(o, elevation_cm) { return { ...o, y: elevation_cm + landmarkHeight(o) / 2 }; }
const geometryCache = new Map();
export function landmarkGeometry(o) {
    const shape = o.shape || 'sphere', pattern = o.pattern || 'solid', w = o.w, h = shape === 'sphere' ? w : o.h, d = ['sphere', 'cylinder'].includes(shape) ? w : o.d;
    const key = JSON.stringify([shape, pattern, w, h, d, o.color, o.pattern_color || '#ffffff', o.pattern_size_cm || 5, o.pattern_tilt_deg || 0]);
    let local = geometryCache.get(key);
    if (!local) {
        local = [];
        const box = (x, y, s, bw, bh, bd, color = o.color) => local.push({ x, y, s, w: bw, h: bh, d: bd, color, kind: 'box' });
        if (shape === 'sphere' && pattern === 'solid')
            local.push({ x: 0, y: 0, s: 0, w, h, d, color: o.color, kind: 'sphere' });
        else if (shape === 'cube' && pattern === 'solid')
            box(0, 0, 0, w, h, d);
        else if (pattern === 'solid') {
            const n = 48;
            for (let i = 0; i < n; i++) {
                if (shape === 'diamond') {
                    const y = -h / 2 + (i + .5) * h / n;
                    box(0, y, 0, w * (1 - Math.abs(y) / (h / 2)), h / n, d);
                }
                else {
                    const s = -d / 2 + (i + .5) * d / n;
                    box(0, 0, s, 2 * Math.sqrt(Math.max(0, (w / 2) ** 2 - s * s)), h, d / n);
                }
            }
        }
        else {
            const n = 24, sx = w / n, sy = h / n, sz = d / n, occupied = new Uint8Array(n * n * n);
            const index = (x, y, z) => x + n * (y + n * z);
            const inside = (x, y, z) => x >= 0 && y >= 0 && z >= 0 && x < n && y < n && z < n && occupied[index(x, y, z)];
            for (let z = 0; z < n; z++)
                for (let y = 0; y < n; y++)
                    for (let x = 0; x < n; x++) {
                        const u = (x + .5) / n * 2 - 1, v = (y + .5) / n * 2 - 1, t = (z + .5) / n * 2 - 1;
                        if (shape === 'cube' || (shape === 'sphere' ? u * u + v * v + t * t <= 1 : shape === 'cylinder' ? u * u + t * t <= 1 : Math.abs(u) + Math.abs(v) <= 1))
                            occupied[index(x, y, z)] = 1;
                    }
            function colorAt(x, y, z) {
                const point = [-w / 2 + (x + .5) * sx, -h / 2 + (y + .5) * sy, -d / 2 + (z + .5) * sz];
                return surfacePattern(o, point) ? (o.pattern_color || '#ffffff') : o.color;
            }
            const shell = (x, y, z) => inside(x, y, z) && (!inside(x - 1, y, z) || !inside(x + 1, y, z) || !inside(x, y - 1, z) || !inside(x, y + 1, z) || !inside(x, y, z - 1) || !inside(x, y, z + 1));
            // Merge runs on the shell to reduce native primitive count.
            for (let z = 0; z < n; z++)
                for (let y = 0; y < n; y++)
                    for (let x = 0; x < n;) {
                        if (!shell(x, y, z)) {
                            x++;
                            continue;
                        }
                        const start = x, color = colorAt(x, y, z);
                        x++;
                        while (x < n && shell(x, y, z) && colorAt(x, y, z) === color)
                            x++;
                        box(-w / 2 + (start + x) * sx / 2, -h / 2 + (y + .5) * sy, -d / 2 + (z + .5) * sz, (x - start) * sx, sy, sz, color);
                    }
        }
        if (geometryCache.size >= 32)
            geometryCache.delete(geometryCache.keys().next().value);
        geometryCache.set(key, local);
    }
    const a = o.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
    const preview_landmark = { shape, x: o.x, y: o.y, s: o.s, w, h, d, angle: o.angle };
    return local.map((part, i) => ({ ...o, ...part, preview_landmark, id: o.id, name: `${o.name.slice(0, 100)} ${i + 1}`, x: o.x + c * part.x - s * part.s, s: o.s + s * part.x + c * part.s, y: o.y + part.y, angle: o.angle }));
}
