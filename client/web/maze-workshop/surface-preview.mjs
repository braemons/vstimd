import { surfaceDimensions, intersectSurface, surfacePattern } from './surfaces.mjs';
// Four subpixel samples keep silhouettes and pattern boundaries smooth.
export class SurfacePreview {
    render(objects, camera, width, height, data, depth, toCamera) {
        const count = width * height;
        if (this.count !== count) {
            this.count = count;
            this.sampleDepth = new Float64Array(count * 4);
            this.samples = new Uint8ClampedArray(count * 16);
            this.stamps = new Uint32Array(count);
            this.frame = 0;
        }
        this.frame++;
        if (this.frame === 0xffffffff) {
            this.stamps.fill(0);
            this.frame = 1;
        }
        const touched = [], yaw = camera.yaw * Math.PI / 180, pitch = camera.pitch || 0, cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch), f = height / 2 / Math.tan(camera.fov * Math.PI / 360);
        const offsets = [[.25, .25], [.75, .25], [.25, .75], [.75, .75]];
        for (const o of objects) {
            const shape = o.shape || 'sphere', dimensions = surfaceDimensions(o), patternSource = { ...o, _surface_dimensions: dimensions, _tiltCos: Math.cos((o.pattern_tilt_deg || 0) * Math.PI / 180), _tiltSin: Math.sin((o.pattern_tilt_deg || 0) * Math.PI / 180) }, a = o.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
            const dx = camera.x - o.x, dy = camera.y - o.y, ds = camera.s - o.s, origin = [c * dx + s * ds, dy, -s * dx + c * ds];
            const corners = [];
            for (const x of [-1, 1])
                for (const y of [-1, 1])
                    for (const z of [-1, 1])
                        corners.push(toCamera([o.x + c * x * dimensions.w / 2 - s * z * dimensions.d / 2, o.y + y * dimensions.h / 2, o.s + s * x * dimensions.w / 2 + c * z * dimensions.d / 2]));
            if (corners.every(p => p[2] < 1))
                continue;
            let left = 0, right = width - 1, top = 0, bottom = height - 1;
            if (corners.every(p => p[2] >= 1)) {
                const xs = corners.map(p => width / 2 + p[0] * f / p[2]), ys = corners.map(p => height / 2 - p[1] * f / p[2]);
                left = Math.max(0, Math.floor(Math.min(...xs)));
                right = Math.min(width - 1, Math.ceil(Math.max(...xs)));
                top = Math.max(0, Math.floor(Math.min(...ys)));
                bottom = Math.min(height - 1, Math.ceil(Math.max(...ys)));
            }
            const base = o.color.match(/\w\w/g).map(v => parseInt(v, 16)), ink = (o.pattern_color || '#ffffff').match(/\w\w/g).map(v => parseInt(v, 16));
            const direction = [0, 0, 0], hitBuffer = { point: [0, 0, 0], normal: [0, 0, 0] };
            for (let py = top; py <= bottom; py++)
                for (let px = left; px <= right; px++) {
                    const index = py * width + px;
                    for (let sample = 0; sample < 4; sample++) {
                        const [ox, oy] = offsets[sample], u = (px + ox - width / 2) / f, v = (height / 2 - py - oy) / f, forward = sp * v + cp;
                        const wx = cy * u + sy * forward, wy = cp * v - sp, ws = -sy * u + cy * forward;
                        direction[0] = c * wx + s * ws;
                        direction[1] = wy;
                        direction[2] = -s * wx + c * ws;
                        const hit = intersectSurface(shape, dimensions, origin, direction, 1, hitBuffer);
                        if (!hit)
                            continue;
                        const inverse = 1 / hit.t;
                        if (this.stamps[index] !== this.frame) {
                            if (inverse <= depth[index])
                                continue;
                            this.stamps[index] = this.frame;
                            touched.push(index);
                            for (let j = 0; j < 4; j++) {
                                this.sampleDepth[index * 4 + j] = depth[index];
                                this.samples.set(data.subarray(index * 4, index * 4 + 4), index * 16 + j * 4);
                            }
                        }
                        const slot = index * 4 + sample;
                        if (inverse <= this.sampleDepth[slot])
                            continue;
                        const color = surfacePattern(patternSource, hit.point, hit.normal) ? ink : base;
                        const nx = c * hit.normal[0] - s * hit.normal[2], ny = hit.normal[1], ns = s * hit.normal[0] + c * hit.normal[2];
                        const diffuse = Math.max(0, (-.4 * nx + .7 * ny - .6 * ns) / Math.hypot(.4, .7, .6));
                        const rayNorm = Math.hypot(wx, wy, ws), hx = -.4 - wx / rayNorm, hy = .7 - wy / rayNorm, hs = -.6 - ws / rayNorm;
                        const specular = Math.pow(Math.max(0, (nx * hx + ny * hy + ns * hs) / (Math.hypot(hx, hy, hs) || 1)), 64) * 28;
                        const factor = .48 + .52 * diffuse, p = index * 16 + sample * 4;
                        this.samples[p] = color[0] * factor + specular;
                        this.samples[p + 1] = color[1] * factor + specular;
                        this.samples[p + 2] = color[2] * factor + specular;
                        this.samples[p + 3] = 255;
                        this.sampleDepth[slot] = inverse;
                    }
                }
        }
        for (const index of touched) {
            let alpha = 0, r = 0, g = 0, b = 0, maxDepth = 0;
            for (let j = 0; j < 4; j++) {
                const p = index * 16 + j * 4, weight = this.samples[p + 3] / 255;
                alpha += weight;
                r += this.samples[p] * weight;
                g += this.samples[p + 1] * weight;
                b += this.samples[p + 2] * weight;
                maxDepth = Math.max(maxDepth, this.sampleDepth[index * 4 + j]);
            }
            if (alpha) {
                const p = index * 4;
                data[p] = r / alpha;
                data[p + 1] = g / alpha;
                data[p + 2] = b / alpha;
                data[p + 3] = alpha / 4 * 255;
                depth[index] = maxDepth;
            }
        }
    }
}
