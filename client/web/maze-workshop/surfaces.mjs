// Analytic surfaces for the editor preview. Coordinates are local cm, Y up.
export function surfaceDimensions(o) { const shape = o.shape || 'sphere'; return { w: o.w, h: shape === 'sphere' ? o.w : o.h, d: ['sphere', 'cylinder'].includes(shape) ? o.w : o.d }; }
export function intersectSurface(shape, dimensions, origin, direction, near = 1, out = null) {
    const { w, h, d } = dimensions, rx = w / 2, ry = h / 2, rz = d / 2;
    if (shape === 'sphere') {
        const x = origin[0], y = origin[1], z = origin[2], dx = direction[0], dy = direction[1], dz = direction[2];
        const a = dx * dx + dy * dy + dz * dz, b = x * dx + y * dy + z * dz, c = x * x + y * y + z * z - rx * rx, discriminant = b * b - a * c;
        if (discriminant < 0 || a < 1e-16)
            return null;
        const root = Math.sqrt(discriminant), low = (-b - root) / a, high = (-b + root) / a, t = low >= near ? low : high >= near ? high : null;
        if (t === null)
            return null;
        const result = out || { point: [0, 0, 0], normal: [0, 0, 0] };
        result.t = t;
        for (let i = 0; i < 3; i++) {
            result.point[i] = origin[i] + t * direction[i];
            result.normal[i] = result.point[i] / rx;
        }
        return result;
    }
    let low = -Infinity, high = Infinity;
    const interval = (a, b) => { low = Math.max(low, a); high = Math.min(high, b); return low <= high; };
    const slab = (p, v, r) => Math.abs(v) < 1e-12 ? Math.abs(p) <= r : interval(Math.min((-r - p) / v, (r - p) / v), Math.max((-r - p) / v, (r - p) / v));
    if (shape === 'sphere' || shape === 'cylinder') {
        const [x, y, z] = origin, [dx, dy, dz] = direction;
        const a = dx * dx + dz * dz + (shape === 'sphere' ? dy * dy : 0), b = x * dx + z * dz + (shape === 'sphere' ? y * dy : 0), c = x * x + z * z + (shape === 'sphere' ? y * y : 0) - rx * rx;
        if (a < 1e-16) {
            if (c > 0)
                return null;
        }
        else {
            const discriminant = b * b - a * c;
            if (discriminant < 0)
                return null;
            const root = Math.sqrt(discriminant);
            if (!interval((-b - root) / a, (-b + root) / a))
                return null;
        }
        if (shape === 'cylinder' && !slab(y, dy, ry))
            return null;
    }
    else if (shape === 'cube') {
        for (let i = 0; i < 3; i++)
            if (!slab(origin[i], direction[i], [rx, ry, rz][i]))
                return null;
    }
    else if (shape === 'diamond') {
        if (!slab(origin[2], direction[2], rz))
            return null;
        for (const sx of [-1, 1])
            for (const sy of [-1, 1]) {
                const p = sx * origin[0] / rx + sy * origin[1] / ry, v = sx * direction[0] / rx + sy * direction[1] / ry;
                if (Math.abs(v) < 1e-12) {
                    if (p > 1)
                        return null;
                }
                else {
                    const t = (1 - p) / v;
                    if (v > 0)
                        high = Math.min(high, t);
                    else
                        low = Math.max(low, t);
                    if (low > high)
                        return null;
                }
            }
    }
    else
        return null;
    const t = low >= near ? low : high >= near ? high : null;
    if (t === null || !Number.isFinite(t))
        return null;
    const point = origin.map((v, i) => v + t * direction[i]);
    let normal;
    if (shape === 'sphere')
        normal = point.map(v => v / rx);
    else if (shape === 'cylinder')
        normal = Math.abs(Math.abs(point[1]) - ry) < 1e-6 * Math.max(1, ry) ? [0, Math.sign(point[1]), 0] : [point[0] / rx, 0, point[2] / rx];
    else if (shape === 'diamond')
        normal = Math.abs(Math.abs(point[2]) - rz) < 1e-6 * Math.max(1, rz) ? [0, 0, Math.sign(point[2])] : [Math.sign(point[0]) / rx, Math.sign(point[1]) / ry, 0];
    else {
        const relative = point.map((v, i) => Math.abs(v) / [rx, ry, rz][i]), axis = relative.indexOf(Math.max(...relative));
        normal = [0, 0, 0];
        normal[axis] = Math.sign(point[axis]);
    }
    const norm = Math.hypot(...normal) || 1;
    return { t, point, normal: normal.map(v => v / norm) };
}
const fract = v => v - Math.floor(v);
export function surfacePattern(o, point, normal = null) {
    const pattern = o.pattern || 'solid';
    if (pattern === 'solid')
        return 0;
    const shape = o.shape || 'sphere', { w, h, d } = o._surface_dimensions || surfaceDimensions(o), size = o.pattern_size_cm || 5;
    let [x, y, z] = point;
    const tilt = (o.pattern_tilt_deg || 0) * Math.PI / 180, c = o._tiltCos ?? Math.cos(tilt), s = o._tiltSin ?? Math.sin(tilt);
    if (shape === 'sphere') {
        const rotatedX = c * x - s * y;
        y = s * x + c * y;
        x = rotatedX;
    }
    let u = x / size, v = y / size, phase;
    if (shape === 'sphere') {
        const r = w / 2, angle = pattern === 'bands' ? 0 : Math.atan2(x, -z), latitude = Math.asin(Math.max(-1, Math.min(1, y / r)));
        if (pattern === 'spiral' || pattern === 'bands') {
            phase = latitude * r / size + (pattern === 'spiral' ? angle / (Math.PI * 2) : 0);
            return fract(phase) < .5 ? 0 : 1;
        }
        const around = Math.max(2, 2 * Math.round(Math.PI * r / size));
        u = (angle / (Math.PI * 2) + .5) * around + .25;
        v = (latitude / Math.PI + .5) * Math.max(1, Math.round(Math.PI * r / size));
    }
    else if (shape === 'cylinder' && Math.abs(y) < h / 2 - 1e-6 * Math.max(1, h)) {
        const r = w / 2, angle = Math.atan2(x, -z);
        u = (angle / (Math.PI * 2) + .5) * Math.max(2, 2 * Math.round(Math.PI * r / size)) + .25;
        v = y / size;
        if (pattern === 'spiral')
            return fract(y / size + angle / (Math.PI * 2)) < .5 ? 0 : 1;
    }
    else {
        const n = normal || [x / (w / 2), y / (h / 2), z / (d / 2)];
        if (Math.abs(n[1]) >= Math.abs(n[0]) && Math.abs(n[1]) >= Math.abs(n[2])) {
            u = x / size;
            v = z / size;
        }
        else if (Math.abs(n[0]) > Math.abs(n[2])) {
            u = z / size;
            v = y / size;
        }
    }
    // Rotate the planar pattern without rotating the object itself.
    if (shape !== 'sphere') {
        const ru = c * u - s * v;
        v = s * u + c * v;
        u = ru;
    }
    if (pattern === 'bands')
        return fract(v) < .5 ? 0 : 1;
    if (pattern === 'spiral')
        return fract(Math.hypot(u, v) + Math.atan2(v, u) / (Math.PI * 2)) < .5 ? 0 : 1;
    const value = pattern === 'diamonds' ? Math.floor(u + v) + Math.floor(u - v) : Math.floor(u) + Math.floor(v);
    return ((value % 2) + 2) % 2;
}
