import { previewTrackLength, previewTrackProgress } from './wheel.mjs';
// Same swept-volume contract as vstimd/dev/CAMERA_MOVEMENT.md. Units: cm.
import { isLandmark, landmarkHeight } from './landmarks.mjs';
export function cameraMovement(project) {
    const obstacles = [];
    for (const o of project.objects) {
        const box = (x, y, s, w, h, d) => obstacles.push({ type: 'Box', position_cm: [x, y, -s], yaw_deg: o.angle, size_cm: [w, h, d] });
        if (o.kind === 'floor')
            continue;
        if (isLandmark(o)) {
            const shape = o.shape || 'sphere';
            if (shape === 'sphere')
                obstacles.push({ type: 'Sphere', position_cm: [o.x, o.y, -o.s], diameter_cm: o.w });
            else
                obstacles.push({ type: { cube: 'Box', diamond: 'Diamond', cylinder: 'Cylinder' }[shape], position_cm: [o.x, o.y, -o.s], yaw_deg: o.angle, size_cm: [o.w, landmarkHeight(o), shape === 'cylinder' ? o.w : o.d] });
        }
        else if (o.kind === 'arch') {
            const a = o.angle * Math.PI / 180;
            for (const dx of [-o.w / 2 - 2, o.w / 2 + 2])
                box(o.x + dx * Math.cos(a), o.y + o.h / 2, o.s + dx * Math.sin(a), 4, o.h, o.d);
            box(o.x, o.y + o.h + 2, o.s, o.w + 8, 4, o.d);
        }
        else
            box(o.x, o.y + o.h / 2, o.s, o.w, o.h, o.d);
    }
    return { start_position_cm: [project.camera.x, project.camera.y, -project.camera.s], start_yaw_deg: -project.camera.yaw, clearance_cm: 1, end_blackout_ms: 2000, obstacles };
}
const collisionDot = (a, b) => a.reduce((v, x, i) => v + x * b[i], 0), collisionSub = (a, b) => a.map((x, i) => x - b[i]), collisionLength = a => Math.hypot(...a);
function obstacleHit(o, from, delta, margin) {
    const a = (o.yaw_deg || 0) * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
    const local = v => [c * v[0] - s * v[2], v[1], s * v[0] + c * v[2]];
    const p = local(collisionSub(from, o.position_cm)), d = local(delta), half = o.size_cm?.map(v => v / 2);
    let lo = -Infinity, hi = Infinity;
    function plane(n, bound) { const a = collisionDot(n, p) - bound, b = collisionDot(n, d); if (Math.abs(b) < 1e-12) {
        if (a >= -1e-10)
            hi = -Infinity;
    }
    else if (b > 0)
        hi = Math.min(hi, -a / b);
    else
        lo = Math.max(lo, -a / b); }
    function slab(n, e) { plane(n, e); plane(n.map(v => -v), e); }
    function ball(p, d, r) {
        const a = collisionDot(d, d), b = collisionDot(p, d), cc = collisionDot(p, p) - r * r;
        if (a < 1e-24) {
            if (cc >= -1e-10)
                hi = -Infinity;
            return;
        }
        const disc = b * b - a * cc;
        if (disc <= 0) {
            hi = -Infinity;
            return;
        }
        lo = Math.max(lo, (-b - Math.sqrt(disc)) / a);
        hi = Math.min(hi, (-b + Math.sqrt(disc)) / a);
    }
    if (o.type === 'Sphere')
        ball(p, d, o.diameter_cm / 2 + margin);
    else if (o.type === 'Cylinder') {
        ball([p[0], 0, p[2]], [d[0], 0, d[2]], half[0] + margin);
        slab([0, 1, 0], half[1] + margin);
    }
    else if (o.type === 'Diamond') {
        for (const x of [-1, 1])
            for (const y of [-1, 1]) {
                const n = [x / half[0], y / half[1], 0];
                plane(n, 1 + margin * collisionLength(n));
            }
        slab([0, 0, 1], half[2] + margin);
    }
    else {
        slab([1, 0, 0], half[0] + margin);
        slab([0, 1, 0], half[1] + margin);
        slab([0, 0, 1], half[2] + margin);
    }
    return hi > Math.max(lo, 0) + 1e-10 && lo < 1 && hi > 1e-10 ? Math.max(lo, 0) : null;
}
export function validateMovementStart(movement) {
    if (movement.obstacles.some(o => obstacleHit(o, movement.start_position_cm, [0, 0, 0], movement.clearance_cm) !== null))
        throw new Error('Initial camera overlaps a wall or landmark (including 1 cm clearance). Move the initial camera to free space.');
}
export function constrainMovement(m, from, to) {
    const delta = collisionSub(to, from), distance = collisionLength(delta);
    if (distance < 1e-12)
        return [...from];
    const yaw = m.start_yaw_deg * Math.PI / 180, forward = [-Math.sin(yaw), 0, -Math.cos(yaw)];
    const along = collisionDot(collisionSub(from, m.start_position_cm), forward), step = collisionDot(delta, forward);
    let t = step < 0 ? Math.min(1, Math.max(0, along) / -step) : 1;
    for (const o of m.obstacles) {
        const hit = obstacleHit(o, from, delta, m.clearance_cm);
        if (hit !== null)
            t = Math.min(t, Math.max(0, hit - .001 / distance));
    }
    return from.map((v, i) => v + delta[i] * t);
}
export function constrainPreview(project, from, to) {
    const end = previewTrackLength(project), along = previewTrackProgress(project.camera, from), step = previewTrackProgress(project.camera, to) - along;
    if (end !== null && step > 0 && along + step > end) {
        const t = Math.max(0, (end - along) / step);
        to = { x: from.x + (to.x - from.x) * t, s: from.s + (to.s - from.s) * t };
    }
    const point = constrainMovement(cameraMovement(project), [from.x, project.camera.y, -from.s], [to.x, project.camera.y, -to.s]);
    return { x: point[0], s: -point[2] };
}
