import { SurfacePreview } from './surface-preview.mjs';
import { corners } from './model.mjs';
// Per-pixel depth keeps a long wall/floor consistent with the same shape tiled
// into smaller pieces. There is no distance-based object loading or culling.
export class DepthPreview {
    render(objects, camera, width, height) {
        width = Math.max(1, Math.round(width));
        height = Math.max(1, Math.round(height));
        if (this.width !== width || this.height !== height) {
            this.width = width;
            this.height = height;
            this.data = new Uint8ClampedArray(width * height * 4);
            this.depth = new Float32Array(width * height);
        }
        this.data.fill(0);
        this.depth.fill(0);
        const yaw = camera.yaw * Math.PI / 180, pitch = camera.pitch || 0;
        const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
        const focal = height / 2 / Math.tan(camera.fov * Math.PI / 360);
        const toCamera = ([x, y, s]) => {
            const dx = x - camera.x, dy = y - camera.y, ds = s - camera.s, forward = sy * dx + cy * ds;
            return [cy * dx - sy * ds, cp * dy + sp * forward, -sp * dy + cp * forward];
        };
        const face = (points, color, factor) => {
            const vertices = points.map(toCamera), clipped = [];
            for (let i = 0; i < vertices.length; i++) {
                const a = vertices[i], b = vertices[(i + 1) % vertices.length], inside = a[2] >= 1, next = b[2] >= 1;
                if (inside)
                    clipped.push(a);
                if (inside !== next) {
                    const t = (1 - a[2]) / (b[2] - a[2]);
                    clipped.push(a.map((v, k) => v + (b[k] - v) * t));
                }
            }
            if (clipped.length < 3)
                return;
            const projected = clipped.map(([x, y, z]) => [width / 2 + x * focal / z, height / 2 - y * focal / z, 1 / z]);
            const rgb = color.map(v => Math.round(v * factor));
            for (let i = 1; i < projected.length - 1; i++)
                this.triangle(projected[0], projected[i], projected[i + 1], rgb);
        };
        const smooth = [];
        for (const o of objects) {
            if (o.kind === 'analytic-landmark') {
                smooth.push(o);
                continue;
            }
            const rgb = o.color.match(/\w\w/g).map(v => parseInt(v, 16));
            const shadedFace = (points, factor) => {
                const source = o.preview_landmark;
                if (source) {
                    const center = points.reduce((sum, p) => sum.map((v, i) => v + p[i] / points.length), [0, 0, 0]);
                    const dx = center[0] - source.x, dy = center[1] - source.y, ds = center[2] - source.s, a = source.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
                    const x = c * dx + s * ds, z = -s * dx + c * ds;
                    let nx = 0, ny = 0, nz = 0;
                    if (source.shape === 'sphere') {
                        nx = x;
                        ny = dy;
                        nz = z;
                    }
                    else if (source.shape === 'cylinder') {
                        if (Math.abs(dy) > source.h * .46)
                            ny = Math.sign(dy);
                        else {
                            nx = x;
                            nz = z;
                        }
                    }
                    else if (source.shape === 'diamond') {
                        if (Math.abs(z) > source.d * .45)
                            nz = Math.sign(z);
                        else {
                            nx = Math.sign(x) / source.w;
                            ny = Math.sign(dy) / source.h;
                        }
                    }
                    else {
                        const ux = Math.abs(x) / source.w, uy = Math.abs(dy) / source.h, uz = Math.abs(z) / source.d;
                        if (uy >= ux && uy >= uz)
                            ny = Math.sign(dy);
                        else if (ux > uz)
                            nx = Math.sign(x);
                        else
                            nz = Math.sign(z);
                    }
                    const worldX = c * nx - s * nz, worldS = s * nx + c * nz, norm = Math.hypot(nx, ny, nz) || 1;
                    factor = .6 + .4 * Math.max(0, (-.4 * worldX + .7 * ny - .6 * worldS) / norm);
                }
                face(points, rgb, factor);
            };
            if (o.kind === 'sphere') {
                const r = o.w / 2, n = 12, m = 8;
                const point = (lat, lon) => [o.x + r * Math.cos(lat) * Math.cos(lon), o.y + r * Math.sin(lat), o.s + r * Math.cos(lat) * Math.sin(lon)];
                for (let i = 0; i < m; i++)
                    for (let j = 0; j < n; j++) {
                        const a = -Math.PI / 2 + i * Math.PI / m, b = a + Math.PI / m, c = j * Math.PI * 2 / n, d = c + Math.PI * 2 / n;
                        face([point(a, c), point(a, d), point(b, d), point(b, c)], rgb, .75 + .25 * i / m);
                    }
            }
            else {
                const cs = corners(o), lo = cs.map(([x, s]) => [x, o.y - o.h / 2, s]), hi = cs.map(([x, s]) => [x, o.y + o.h / 2, s]);
                shadedFace(hi, 1);
                shadedFace(lo, .5);
                for (let i = 0; i < 4; i++) {
                    const j = (i + 1) % 4;
                    shadedFace([lo[i], lo[j], hi[j], hi[i]], .62 + i * .08);
                }
            }
        }
        if (smooth.length) {
            this.surfaces ??= new SurfacePreview();
            this.surfaces.render(smooth, camera, width, height, this.data, this.depth, toCamera);
        }
        return { width, height, data: this.data };
    }
    triangle(a, b, c, rgb) {
        const denominator = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if (Math.abs(denominator) < 1e-10)
            return;
        const left = Math.max(0, Math.ceil(Math.min(a[0], b[0], c[0]) - .5));
        const right = Math.min(this.width - 1, Math.floor(Math.max(a[0], b[0], c[0]) - .5));
        const top = Math.max(0, Math.ceil(Math.min(a[1], b[1], c[1]) - .5));
        const bottom = Math.min(this.height - 1, Math.floor(Math.max(a[1], b[1], c[1]) - .5));
        if (left > right || top > bottom)
            return;
        const ax = (b[1] - c[1]) / denominator, ay = (c[0] - b[0]) / denominator;
        const bx = (c[1] - a[1]) / denominator, by = (a[0] - c[0]) / denominator;
        let rowA = ax * (left + .5 - c[0]) + ay * (top + .5 - c[1]);
        let rowB = bx * (left + .5 - c[0]) + by * (top + .5 - c[1]);
        const za = a[2] - c[2], zb = b[2] - c[2], stepZ = ax * za + bx * zb;
        for (let y = top; y <= bottom; y++) {
            let wa = rowA, wb = rowB, z = wa * za + wb * zb + c[2], index = y * this.width + left;
            for (let x = left; x <= right; x++, index++, wa += ax, wb += bx, z += stepZ) {
                if (wa >= -1e-8 && wb >= -1e-8 && wa + wb <= 1 + 1e-8 && z > this.depth[index]) {
                    this.depth[index] = z;
                    const p = index * 4;
                    this.data[p] = rgb[0];
                    this.data[p + 1] = rgb[1];
                    this.data[p + 2] = rgb[2];
                    this.data[p + 3] = 255;
                }
            }
            rowA += ay;
            rowB += by;
        }
    }
}
