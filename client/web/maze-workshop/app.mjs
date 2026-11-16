import { installPreviewResizer } from './resizer.mjs';
import { installShortcutHelp } from './shortcuts.mjs';
import { ReturnHold } from './return.mjs';
import { constrainPreview, cameraMovement, validateMovementStart } from './collisions.mjs';
import { validateNavigation, suggestTrackLength, previewTrackLength, previewTrackProgress } from './wheel.mjs';
import { minimapTransform, worldToMinimap, minimapToWorld, previewOffsets } from './navigation.mjs';
import { isLandmark, isRoundLandmark, landmarkFootprint, landmarkHeight, landmarkElevation, landmarkAtElevation } from './landmarks.mjs';
import { planMeasurements, rulerDistance, snapRulerPoint } from './measurements.mjs';
import { DepthPreview } from './preview.mjs';
import { clone, uid, object, template, validate, exportScene, solids, corners, hit, distributeEvenly, wallPanels, resizeCorners, resizeFromCorner, objectsInsideRect, backgroundColor } from './model.mjs';
const $ = s => document.querySelector(s), plan = $('#plan'), preview = $('#preview'), minimap = $('#minimap');
const key = 'braemons.maze-editor.v1';
const previewReturn = new ReturnHold();
let returnTimer = null;
let project = template(), selected = null, tool = 'select', history = [], future = [], drag = null, hover = null, grid = 10, view = { x: 0, s: 200, scale: 1 }, mode = 'orbit', walk = 0, walkX = 0, turn = 0;
const defaultOrbit = () => ({ azimuth: -32 * Math.PI / 180, elevation: 30 * Math.PI / 180, distance: 1 });
let orbit = defaultOrbit(), orbitDrag = null;
let overviewFocus = null, minimapView = null, navigationTrail = [];
let copiedObjects = null, pointerClient = null;
let selectedIds = new Set(), cameraSelected = false;
let dimensionsVisible = false;
let ruler = null;
const selectedObjects = () => project.objects.filter(o => selectedIds.has(o.id));
function selectOnly(id) { cameraSelected = false; selectedIds = new Set(id ? [id] : []); selected = id; }
function toggleSelection(id) { cameraSelected = false; if (selectedIds.has(id))
    selectedIds.delete(id);
else
    selectedIds.add(id); selected = [...selectedIds].at(-1) || null; }
function reconcileSelection() { selectedIds = new Set([...selectedIds].filter(id => project.objects.some(o => o.id === id))); if (!selectedIds.has(selected))
    selected = [...selectedIds].at(-1) || null; }
let storageAvailable = true;
try {
    const saved = localStorage.getItem(key);
    if (saved)
        project = validate(JSON.parse(saved));
}
catch {
    storageAvailable = false;
}
let workshopRig = null, rigBusy = false, lastRigSave = null;
let toastTimer;
function toast(message) { $('#toast').textContent = message; $('#toast').style.display = 'block'; clearTimeout(toastTimer); toastTimer = setTimeout(() => $('#toast').style.display = 'none', 4500); }
function persist() { try {
    localStorage.setItem(key, JSON.stringify(project));
    storageAvailable = true;
}
catch {
    storageAvailable = false;
} }
function change(fn) { cancelPreviewReturn(); const before = clone(project), selectionBefore = [...selectedIds], primaryBefore = selected; try {
    fn();
    project = validate(project);
    history.push(before);
    if (history.length > 60)
        history.shift();
    future = [];
    persist();
    refresh();
    return true;
}
catch (e) {
    project = before;
    selectedIds = new Set(selectionBefore);
    selected = primaryBefore;
    refresh();
    toast(e.message);
    return false;
} }
function undo(redo = false) { cancelPreviewReturn(); const source = redo ? future : history, dest = redo ? history : future; if (!source.length)
    return; dest.push(clone(project)); project = source.pop(); reconcileSelection(); persist(); refresh(); }
function size(canvas) { const r = canvas.getBoundingClientRect(), dpr = window.devicePixelRatio || 1; if (canvas.width !== Math.round(r.width * dpr) || canvas.height !== Math.round(r.height * dpr)) {
    canvas.width = Math.round(r.width * dpr);
    canvas.height = Math.round(r.height * dpr);
} const ctx = canvas.getContext('2d'); ctx.setTransform(dpr, 0, 0, dpr, 0, 0); return { ctx, w: r.width, h: r.height }; }
function worldToScreen(x, s, w, h) { return [w / 2 + (x - view.x) * view.scale, h / 2 - (s - view.s) * view.scale]; }
function resizeHandleHit(e) {
    if (tool !== 'select' || selectedIds.size !== 1)
        return null;
    const o = selectedObjects()[0];
    if (!o)
        return null;
    const rect = plan.getBoundingClientRect(), points = resizeCorners(o);
    let best = null, bestDistance = 10;
    points.forEach((p, index) => { const [x, y] = worldToScreen(...p, rect.width, rect.height), distance = Math.hypot(e.clientX - rect.left - x, e.clientY - rect.top - y); if (distance < bestDistance) {
        bestDistance = distance;
        best = { object: o, index };
    } });
    return best;
}
function resizeCursor(o, index) {
    const [u, v] = [[-1, -1], [1, -1], [1, 1], [-1, 1]][index], a = ((o.kind === 'sphere' || o.shape === 'sphere') ? 0 : o.angle) * Math.PI / 180;
    const angle = Math.atan2(-(Math.sin(a) * u + Math.cos(a) * v), Math.cos(a) * u - Math.sin(a) * v);
    return ['ew-resize', 'nwse-resize', 'ns-resize', 'nesw-resize'][((Math.round(angle / (Math.PI / 4)) % 4) + 4) % 4];
}
function drawResizeHandles(ctx, w, h) {
    if (tool !== 'select' || selectedIds.size !== 1)
        return;
    const o = selectedObjects()[0];
    if (!o)
        return;
    ctx.save();
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = '#315c48';
    ctx.fillStyle = '#fffef9';
    for (const p of resizeCorners(o)) {
        const [x, y] = worldToScreen(...p, w, h);
        ctx.fillRect(x - 4, y - 4, 8, 8);
        ctx.strokeRect(x - 4, y - 4, 8, 8);
    }
    ctx.restore();
}
function cameraHit(e) { const rect = plan.getBoundingClientRect(), [x, y] = worldToScreen(project.camera.x, project.camera.s, rect.width, rect.height); return Math.hypot(e.clientX - rect.left - x, e.clientY - rect.top - y) <= 14; }
function eventPoint(e) { const r = plan.getBoundingClientRect(); return { x: view.x + (e.clientX - r.left - r.width / 2) / view.scale, s: view.s - (e.clientY - r.top - r.height / 2) / view.scale }; }
const snap = v => Math.round(v / grid) * grid;
function bounds() { const points = project.objects.flatMap(corners); points.push([project.camera.x, project.camera.s]); if (points.length === 1)
    points.push([-80, -20], [80, 400]); return { minX: Math.min(...points.map(p => p[0])), maxX: Math.max(...points.map(p => p[0])), minS: Math.min(...points.map(p => p[1])), maxS: Math.max(...points.map(p => p[1])) }; }
function fit() { const b = bounds(), r = plan.getBoundingClientRect(); view = { x: (b.minX + b.maxX) / 2, s: (b.minS + b.maxS) / 2, scale: Math.max(.03, Math.min(5, (r.width - 100) / Math.max(100, b.maxX - b.minX), (r.height - 90) / Math.max(100, b.maxS - b.minS))) }; draw(); }
function polygon(ctx, points, fill, stroke) { ctx.beginPath(); points.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.closePath(); if (fill) {
    ctx.fillStyle = fill;
    ctx.fill();
} if (stroke) {
    ctx.strokeStyle = stroke;
    ctx.stroke();
} }
function drawPlan() {
    const { ctx, w, h } = size(plan);
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#f9faf4';
    ctx.fillRect(0, 0, w, h);
    let spacing = grid;
    while (spacing * view.scale < 12)
        spacing *= 2;
    ctx.strokeStyle = '#e7ebdf';
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = Math.ceil((view.x - w / 2 / view.scale) / spacing) * spacing; x < view.x + w / 2 / view.scale; x += spacing) {
        const [sx] = worldToScreen(x, 0, w, h);
        ctx.moveTo(sx, 0);
        ctx.lineTo(sx, h);
    }
    for (let s = Math.ceil((view.s - h / 2 / view.scale) / spacing) * spacing; s < view.s + h / 2 / view.scale; s += spacing) {
        const [, sy] = worldToScreen(0, s, w, h);
        ctx.moveTo(0, sy);
        ctx.lineTo(w, sy);
    }
    ctx.stroke();
    const ordered = [...project.objects].sort((a, b) => (a.kind === 'floor' ? 0 : 1) - (b.kind === 'floor' ? 0 : 1));
    for (const o of ordered) {
        ctx.lineWidth = selectedIds.has(o.id) ? 2 : 1;
        const stroke = selectedIds.has(o.id) ? '#236344' : o.kind === 'floor' ? '#c1c9bc' : '#ffffff88';
        if (isRoundLandmark(o)) {
            const [x, y] = worldToScreen(o.x, o.s, w, h);
            ctx.beginPath();
            ctx.arc(x, y, Math.max(4, o.w * view.scale / 2), 0, Math.PI * 2);
            ctx.fillStyle = o.color;
            ctx.fill();
            ctx.strokeStyle = stroke;
            ctx.stroke();
        }
        else
            polygon(ctx, corners(o).map(p => worldToScreen(...p, w, h)), o.kind === 'arch' ? o.color + '99' : o.color, stroke);
        if (selectedIds.has(o.id)) {
            ctx.setLineDash([4, 3]);
            polygon(ctx, corners({ ...o, w: o.w + 8 / view.scale, d: o.d + 8 / view.scale }).map(p => worldToScreen(...p, w, h)), null, '#315c48');
            ctx.setLineDash([]);
        }
    }
    const cam = project.camera, [cx, cy] = worldToScreen(cam.x, cam.s, w, h), a = cam.yaw * Math.PI / 180;
    ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(a);
    polygon(ctx, [[-7, 8], [7, 8], [0, -12]], '#315c48', '#fff');
    ctx.strokeStyle = '#315c4850';
    ctx.beginPath();
    ctx.moveTo(0, 0);
    ctx.lineTo(-22, -40);
    ctx.moveTo(0, 0);
    ctx.lineTo(22, -40);
    ctx.stroke();
    ctx.restore();
    if (drag && !drag.marquee && !drag.ruler && !drag.cameraOriginal && !drag.resizeOriginal && tool !== 'select' && hover) {
        const [x, y] = worldToScreen(drag.start.x, drag.start.s, w, h), [ex, ey] = worldToScreen(snap(hover.x), snap(hover.s), w, h);
        ctx.strokeStyle = '#315c48';
        ctx.setLineDash([5, 4]);
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(ex, ey);
        ctx.stroke();
        ctx.setLineDash([]);
    }
    if (dimensionsVisible)
        drawDimensions(ctx, w, h);
    drawResizeHandles(ctx, w, h);
    if (tool === 'ruler' && ruler)
        drawRuler(ctx, w, h);
    if (drag?.marquee) {
        const a = worldToScreen(drag.start.x, drag.start.s, w, h), b = worldToScreen(drag.end.x, drag.end.s, w, h);
        ctx.save();
        ctx.fillStyle = '#5b956222';
        ctx.strokeStyle = '#315c48';
        ctx.lineWidth = 1;
        ctx.setLineDash([5, 3]);
        ctx.fillRect(Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
        ctx.strokeRect(Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
        ctx.restore();
    }
    $('#zoom-label').textContent = Math.round(view.scale * 100) + '%';
}
function rulerEndpoint(e) { const p = eventPoint(e); return e.altKey ? { ...p, snapped: false } : snapRulerPoint(project.objects, p, 9 / view.scale); }
function rulerHint() {
    $('#hint').textContent = !ruler ? 'Click the first point. Snap to object edges; hold Alt for free measurement.' : !ruler.complete ? 'Click the second point, or drag. Hold Alt to disable snapping.' : `Ruler: ${Number(rulerDistance(ruler.from, ruler.to).toFixed(2))} cm. Click to start again; Esc to exit.`;
}
function drawRuler(ctx, w, h) {
    const a = worldToScreen(ruler.from.x, ruler.from.s, w, h), b = worldToScreen(ruler.to.x, ruler.to.s, w, h);
    ctx.save();
    ctx.lineWidth = 2;
    ctx.strokeStyle = '#216c96';
    ctx.fillStyle = '#fffef9';
    ctx.setLineDash(ruler.complete ? [] : [5, 4]);
    ctx.beginPath();
    ctx.moveTo(...a);
    ctx.lineTo(...b);
    ctx.stroke();
    ctx.setLineDash([]);
    for (const p of [a, b]) {
        ctx.beginPath();
        ctx.arc(...p, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
    }
    const label = `${Number(rulerDistance(ruler.from, ruler.to).toFixed(2))} cm`;
    ctx.font = '600 12px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    const labelWidth = ctx.measureText(label).width + 18, x = Math.max(labelWidth / 2 + 3, Math.min(w - labelWidth / 2 - 3, (a[0] + b[0]) / 2)), y = Math.max(14, Math.min(h - 14, (a[1] + b[1]) / 2 - 16));
    ctx.fillRect(x - labelWidth / 2, y - 11, labelWidth, 22);
    ctx.strokeRect(x - labelWidth / 2, y - 11, labelWidth, 22);
    ctx.fillStyle = '#216c96';
    ctx.fillText(label, x, y);
    ctx.restore();
}
function drawDimensions(ctx, w, h) {
    const segments = planMeasurements(project.objects, [...selectedIds]);
    // Selected spacing first, then overall bounds, then object dimensions.
    const rank = s => s.type === 'gap' || s.type === 'position' ? 0 : s.type === 'overall' ? 1 : s.priority ? 2 : 3;
    segments.sort((a, b) => rank(a) - rank(b));
    const labels = [];
    ctx.save();
    ctx.font = '10px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    for (const segment of segments) {
        const a = worldToScreen(...segment.from, w, h), b = worldToScreen(...segment.to, w, h), dx = b[0] - a[0], dy = b[1] - a[1], len = Math.hypot(dx, dy);
        if (len < 1 || (!segment.priority && len < 55))
            continue;
        const nx = -dy / len, ny = dx / len, offset = segment.offset;
        const x0 = a[0] + nx * offset, y0 = a[1] + ny * offset, x1 = b[0] + nx * offset, y1 = b[1] + ny * offset;
        if (Math.max(x0, x1) < 0 || Math.min(x0, x1) > w || Math.max(y0, y1) < 0 || Math.min(y0, y1) > h)
            continue;
        let angle = Math.atan2(dy, dx);
        if (angle > Math.PI / 2 || angle < -Math.PI / 2)
            angle += Math.PI;
        const textWidth = ctx.measureText(segment.label).width + 10, tx = (x0 + x1) / 2, ty = (y0 + y1) / 2;
        const tw = Math.abs(Math.cos(angle)) * textWidth + Math.abs(Math.sin(angle)) * 16, th = Math.abs(Math.sin(angle)) * textWidth + Math.abs(Math.cos(angle)) * 16;
        const rect = { x: tx - tw / 2, y: ty - th / 2, w: tw, h: th };
        if (rect.x < 2 || rect.y < 2 || rect.x + rect.w > w - 2 || rect.y + rect.h > h - 2 || labels.some(r => rect.x < r.x + r.w + 3 && rect.x + rect.w + 3 > r.x && rect.y < r.y + r.h + 3 && rect.y + rect.h + 3 > r.y))
            continue;
        labels.push(rect);
        ctx.lineWidth = 1;
        ctx.strokeStyle = segment.type === 'gap' || segment.type === 'position' ? '#386f91' : '#426c59';
        ctx.setLineDash([2, 3]);
        ctx.beginPath();
        ctx.moveTo(...a);
        ctx.lineTo(x0 + nx * 4, y0 + ny * 4);
        ctx.moveTo(...b);
        ctx.lineTo(x1 + nx * 4, y1 + ny * 4);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.beginPath();
        ctx.moveTo(x0, y0);
        ctx.lineTo(x1, y1);
        for (const [x, y] of [[x0, y0], [x1, y1]]) {
            ctx.moveTo(x - nx * 4 - dx / len * 3, y - ny * 4 - dy / len * 3);
            ctx.lineTo(x + nx * 4 + dx / len * 3, y + ny * 4 + dy / len * 3);
        }
        ctx.stroke();
        ctx.save();
        ctx.translate(tx, ty);
        ctx.rotate(angle);
        ctx.fillStyle = '#fffef9ed';
        ctx.fillRect(-textWidth / 2, -8, textWidth, 16);
        ctx.fillStyle = ctx.strokeStyle;
        ctx.fillText(segment.label, 0, 0);
        ctx.restore();
    }
    ctx.restore();
}
$('#dimensions').onclick = () => { dimensionsVisible = !dimensionsVisible; $('#dimensions').setAttribute('aria-checked', String(dimensionsVisible)); $('#dimensions').classList.toggle('active', dimensionsVisible); $('#dimensions-state').textContent = dimensionsVisible ? 'On' : 'Off'; drawPlan(); };
// Software preview with near-plane clipping and per-pixel depth.
const depthPreview = new DepthPreview(), previewSurface = document.createElement('canvas');
const previewSurfaceContext = previewSurface.getContext('2d');
function drawPreview() {
    const { ctx, w, h } = size(preview);
    if (mode === 'eye' && previewReturn.active) {
        ctx.fillStyle = '#000';
        ctx.fillRect(0, 0, w, h);
        drawMinimap();
        return;
    }
    ctx.fillStyle = backgroundColor(project);
    ctx.fillRect(0, 0, w, h);
    const b = bounds();
    let camera, pitch;
    if (mode === 'orbit') {
        const span = Math.max(160, b.maxX - b.minX, b.maxS - b.minS), radius = (overviewFocus ? 120 : span * 1.45) * orbit.distance, horizontal = radius * Math.cos(orbit.elevation), target = overviewFocus || { x: (b.minX + b.maxX) / 2, s: (b.minS + b.maxS) / 2 };
        camera = { x: target.x + horizontal * Math.sin(orbit.azimuth), s: target.s - horizontal * Math.cos(orbit.azimuth), y: radius * Math.sin(orbit.elevation), yaw: -orbit.azimuth * 180 / Math.PI, fov: 55 };
        pitch = orbit.elevation;
    }
    else {
        camera = { ...project.camera, x: project.camera.x + walkX, s: project.camera.s + walk, yaw: project.camera.yaw + turn };
        pitch = 0;
    }
    const ratio = Math.min(1, 1600 / Math.max(1, w), 800 / Math.max(1, h));
    const image = depthPreview.render(solids(project, { preview: true }), { ...camera, pitch }, w * ratio, h * ratio);
    if (previewSurface.width !== image.width || previewSurface.height !== image.height) {
        previewSurface.width = image.width;
        previewSurface.height = image.height;
    }
    previewSurfaceContext.putImageData(new ImageData(image.data, image.width, image.height), 0, 0);
    ctx.drawImage(previewSurface, 0, 0, w, h);
    ctx.fillStyle = '#f3f5e8b0';
    ctx.font = '10px sans-serif';
    ctx.fillText(mode === 'orbit' ? 'OVERVIEW · PREVIEW' : 'CAMERA · PREVIEW', 17, 22);
    if (!project.objects.length) {
        ctx.font = '16px sans-serif';
        ctx.fillText('Start drawing on the floor plan.', 25, h / 2);
    }
    drawMinimap();
}
function previewPosition() { return { x: project.camera.x + walkX, s: project.camera.s + walk }; }
function syncWalkControl() { const slider = $('#walk'), along = previewTrackProgress(project.camera, previewPosition()), end = previewTrackLength(project); slider.min = 0; slider.max = end ?? Math.max(800, Math.ceil(along)); slider.value = along; $('#distance').value = `${Math.round(along)} cm`; }
function cancelPreviewReturn() { clearTimeout(returnTimer); returnTimer = null; previewReturn.cancel(); }
function completePreviewReturn() {
    if (!previewReturn.active)
        return;
    const now = performance.now();
    if (!previewReturn.finish(now)) {
        returnTimer = setTimeout(completePreviewReturn, Math.max(1, previewReturn.deadline - now));
        return;
    }
    returnTimer = null;
    walk = 0;
    walkX = 0;
    turn = 0;
    overviewFocus = null;
    recordPreviewPosition(true);
    syncWalkControl();
    draw();
}
function movePreviewTo(point) {
    if (previewReturn.active)
        return false;
    const accepted = constrainPreview(project, previewPosition(), point);
    ({ walkX, walk } = previewOffsets(project.camera, accepted));
    recordPreviewPosition();
    syncWalkControl();
    const end = previewTrackLength(project);
    if (end !== null && previewTrackProgress(project.camera, accepted) >= end - 1e-6) {
        previewReturn.begin(performance.now());
        returnTimer = setTimeout(completePreviewReturn, 2000);
    }
    return true;
}
function recordPreviewPosition(newSegment = false) {
    const position = previewPosition(), last = navigationTrail.at(-1);
    if (newSegment && navigationTrail.length)
        navigationTrail.push(null);
    if (newSegment || !last || Math.hypot(last.x - position.x, last.s - position.s) > .1)
        navigationTrail.push(position);
    if (navigationTrail.length > 500)
        navigationTrail = navigationTrail.slice(-500);
}
function resetNavigation() { cancelPreviewReturn(); overviewFocus = null; navigationTrail = []; walk = 0; walkX = 0; turn = 0; orbit = defaultOrbit(); syncWalkControl(); }
function drawMinimap() {
    if ($('#minimap-panel').hidden)
        return;
    const { ctx, w, h } = size(minimap);
    if (w < 1 || h < 1)
        return;
    const b = bounds(), position = previewPosition();
    minimapView = minimapTransform({ ...b, minX: Math.min(b.minX, position.x), maxX: Math.max(b.maxX, position.x), minS: Math.min(b.minS, position.s), maxS: Math.max(b.maxS, position.s) }, w, h);
    const screen = p => { const q = worldToMinimap(p, minimapView); return [q.x, q.y]; };
    ctx.clearRect(0, 0, w, h);
    const objects = [...project.objects].sort((a, b) => (a.kind === 'floor' ? 0 : 1) - (b.kind === 'floor' ? 0 : 1));
    for (const o of objects) {
        ctx.fillStyle = o.color;
        ctx.strokeStyle = '#53695b';
        ctx.lineWidth = .5;
        if (isRoundLandmark(o)) {
            const [x, y] = screen(o);
            ctx.beginPath();
            ctx.arc(x, y, Math.max(1.5, o.w * minimapView.scale / 2), 0, Math.PI * 2);
            ctx.fill();
        }
        else
            polygon(ctx, corners(o).map(([x, s]) => screen({ x, s })), o.color, o.kind === 'floor' ? null : '#53695b');
    }
    ctx.beginPath();
    ctx.strokeStyle = '#3c88ad';
    ctx.lineWidth = 1.5;
    let drawing = false;
    for (const point of navigationTrail) {
        if (!point) {
            drawing = false;
            continue;
        }
        const [x, y] = screen(point);
        if (drawing)
            ctx.lineTo(x, y);
        else
            ctx.moveTo(x, y);
        drawing = true;
    }
    ctx.stroke();
    const [x, y] = screen(position), a = (project.camera.yaw + turn) * Math.PI / 180 + Math.PI / 2;
    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(a);
    polygon(ctx, [[-4, 4], [4, 4], [0, -7]], '#187caa', '#fff');
    ctx.restore();
    if (overviewFocus) {
        const [fx, fy] = screen(overviewFocus);
        ctx.strokeStyle = '#187caa';
        ctx.beginPath();
        ctx.arc(fx, fy, 7, 0, Math.PI * 2);
        ctx.stroke();
    }
}
function navigateMinimap(e) {
    if (e.button !== 0 || !minimapView)
        return;
    e.preventDefault();
    e.stopPropagation();
    const rect = minimap.getBoundingClientRect(), point = minimapToWorld({ x: e.clientX - rect.left, y: e.clientY - rect.top }, minimapView);
    Object.assign(point, { x: Math.max(-10000, Math.min(10000, point.x)), s: Math.max(-10000, Math.min(10000, point.s)) });
    if (!movePreviewTo(point))
        return;
    overviewFocus = { ...previewPosition() };
    orbit.distance = 1;
    drawPreview();
    preview.focus({ preventScroll: true });
}
minimap.addEventListener('pointerdown', navigateMinimap);
$('#toggle-map').onclick = () => { const panel = $('#minimap-panel'); panel.hidden = !panel.hidden; $('#toggle-map').setAttribute('aria-pressed', String(!panel.hidden)); $('#toggle-map').classList.toggle('active', !panel.hidden); if (!panel.hidden)
    drawMinimap(); };
function draw() { drawPlan(); drawPreview(); }
function field(label, value, { type = 'number', step = 1, min, max, mixed = false, onchange } = {}) {
    const l = document.createElement('label');
    l.className = 'field' + (type === 'text' ? ' name' : '');
    const s = document.createElement('span');
    s.textContent = label + (mixed && type === 'color' ? ' · Mixed' : '');
    const input = document.createElement('input');
    input.type = type;
    input.value = mixed && type !== 'color' ? '' : value;
    if (mixed) {
        input.placeholder = 'Mixed';
        input.title = 'Mixed values. Enter a value to apply it to every selected object.';
    }
    if (type === 'number') {
        input.step = step;
        if (min !== undefined)
            input.min = min;
        if (max !== undefined)
            input.max = max;
    }
    if (type === 'text')
        input.maxLength = 120;
    input.addEventListener('change', () => { if (type === 'number' && input.value === '') {
        refresh();
        return;
    } if (!input.checkValidity()) {
        input.reportValidity();
        refresh();
        return;
    } onchange(type === 'number' ? Number(input.value) : input.value); });
    l.append(s, input);
    return l;
}
function objectFields(o) { if (o.kind === 'landmark')
    return [['x', 'X · cm'], ['s', 'Forward · cm'], ['elevation_cm', 'Elevation · cm'], ['w', isRoundLandmark(o) ? 'Diameter · cm' : 'Width · cm'], ...(['cube', 'diamond'].includes(o.shape) ? [['d', 'Depth · cm']] : []), ...(o.shape !== 'sphere' ? [['h', 'Object height · cm']] : []), ['angle', 'Rotation · degrees']]; return [['x', 'X · cm'], ['s', 'Forward · cm'], [o.kind === 'sphere' ? 'elevation_cm' : 'y', o.kind === 'sphere' ? 'Elevation · cm' : 'Base · cm'], ['w', o.kind === 'sphere' ? 'Diameter · cm' : o.kind === 'arch' ? 'Opening · cm' : 'Width · cm'], ...(o.kind === 'sphere' ? [] : [['d', 'Depth · cm'], ...(o.kind === 'floor' ? [] : [['h', o.kind === 'arch' ? 'Clear height · cm' : 'Height · cm']]), ['angle', 'Rotation · degrees']])]; }
function objectProperty(o, key) { return key === 'elevation_cm' ? landmarkElevation(o) : o[key]; }
function applyObjectProperty(o, key, value) {
    if (isLandmark(o)) {
        const elevation = landmarkElevation(o);
        if (key === 'elevation_cm') {
            Object.assign(o, landmarkAtElevation(o, value));
            return;
        }
        o[key] = value;
        if (key === 'w' || key === 'h')
            Object.assign(o, landmarkAtElevation(o, elevation));
    }
    else
        o[key] = value;
}
function syncInspector() {
    const objects = selectedIds.size > 0, camera = cameraSelected || tool === 'camera';
    $('#object-inspector').hidden = !objects;
    $('#camera-inspector').hidden = !camera;
    $('#inspector').hidden = !objects && !camera;
}
function refresh() {
    $('#background-color').value = backgroundColor(project);
    reconcileSelection();
    syncInspector();
    const objects = selectedObjects(), count = objects.length, o = objects[0];
    $('#selection-title').textContent = count > 1 ? `${count} objects selected` : o ? o.name : 'Select an object';
    $('#distribute').disabled = count < 3;
    const props = $('#properties');
    props.replaceChildren();
    if (count) {
        if (count === 1)
            props.append(field('Name', o.name, { type: 'text', onchange: v => change(() => o.name = v) }));
        else {
            const hint = document.createElement('p');
            hint.className = 'muted';
            hint.textContent = 'Changes apply to all selected objects. Mixed means their values differ.';
            props.append(hint);
        }
        const fields = objectFields(o).filter(([key]) => objects.every(item => objectFields(item).some(([k]) => k === key)));
        for (const [key, label] of [...fields, ['color', 'Color']]) {
            const mixed = objects.some(item => objectProperty(item, key) !== objectProperty(o, key));
            const sameLabel = key === 'color' || objects.every(item => objectFields(item).find(([k]) => k === key)?.[1] === label);
            const sharedLabel = sameLabel ? label : ({ y: 'Base / centre · cm', w: 'Width / size · cm', h: 'Height / opening · cm' })[key] || label;
            props.append(field(sharedLabel, objectProperty(o, key), { type: key === 'color' ? 'color' : 'number', step: .1, min: ['w', 'd', 'h'].includes(key) ? .1 : -10000, max: 10000, mixed, onchange: value => change(() => { for (const item of objects)
                    applyObjectProperty(item, key, value); }) }));
        }
        if (count === 1 && isLandmark(o)) {
            const edit = document.createElement('button');
            edit.className = 'full';
            edit.textContent = 'Shape & pattern';
            edit.onclick = () => openLandmarkDialog(o.id);
            props.append(edit);
        }
        if (count === 1 && o.kind === 'wall') {
            const add = document.createElement('button');
            add.className = 'full';
            add.textContent = 'Add panels';
            add.onclick = () => openPanelDialog(o.id);
            props.append(add);
        }
        const row = document.createElement('div');
        row.className = 'row';
        for (const [text, fn] of [['Duplicate', duplicate], ['Delete', remove]]) {
            const button = document.createElement('button');
            button.textContent = text;
            button.onclick = fn;
            row.append(button);
        }
        props.append(row);
    }
    else {
        const hint = document.createElement('p');
        hint.className = 'muted';
        hint.textContent = 'Select an object to edit. Shift + click adds or removes objects.';
        props.append(hint);
    }
    const cp = $('#camera-properties');
    cp.replaceChildren();
    for (const [k, label] of [['x', 'X · cm'], ['s', 'Forward · cm'], ['y', 'Height · cm'], ['yaw', 'Heading · degrees'], ['fov', 'Vertical FOV · degrees']])
        cp.append(field(label, project.camera[k], { min: k === 'fov' ? 20 : k === 'y' ? 1 : -10000, max: k === 'fov' ? 120 : 10000, onchange: v => change(() => project.camera[k] = v) }));
    const list = $('#object-list');
    list.replaceChildren();
    const filter = $('#filter').value.toLowerCase();
    for (const o of project.objects.filter(o => o.name.toLowerCase().includes(filter))) {
        const button = document.createElement('button');
        if (selectedIds.has(o.id))
            button.className = 'active';
        const swatch = document.createElement('span');
        swatch.className = 'swatch';
        swatch.style.background = o.color;
        const name = document.createElement('span');
        name.className = 'item-name';
        name.textContent = o.name;
        button.append(swatch, name);
        button.onclick = e => { if (e.shiftKey)
            toggleSelection(o.id);
        else
            selectOnly(o.id); setTool('select'); refresh(); };
        list.append(button);
    }
    $('#count').textContent = project.objects.length;
    $('#undo').disabled = !history.length;
    $('#redo').disabled = !future.length;
    draw();
}
function setTool(t) { if (t !== tool)
    ruler = null; tool = t; if (t === 'camera') {
    selectOnly(null);
    cameraSelected = true;
}
else
    cameraSelected = false; for (const button of document.querySelectorAll('[data-tool]')) {
    button.classList.toggle('active', button.dataset.tool === t);
    button.setAttribute('aria-pressed', String(button.dataset.tool === t));
} syncInspector(); $('#hint').textContent = ({ select: 'Drag background to box-select. Ctrl + drag to pan. Shift adds to selection.', wall: 'Drag between two points to draw a wall.', floor: 'Drag opposite corners to draw a floor.', landmark: 'Click the plan to place the landmark.', arch: 'Click to place an arch. Adjust its opening and rotation.', camera: 'Click to place the initial camera.', ruler: 'Click two points or drag to measure in cm. Snap to edges; Alt disables snapping.' })[t]; plan.style.cursor = t === 'select' ? 'default' : 'crosshair'; drawPlan(); }
let landmarkConfig = { shape: 'sphere', pattern: 'solid', pattern_color: '#ffffff', pattern_size_cm: 5, pattern_tilt_deg: 0, color: '#437aff', w: 30, d: 30, h: 30, y: 15 };
let landmarkEditingId = null;
const landmarkPreview = new DepthPreview();
function landmarkOptions() {
    const shape = $('#landmark-shape').value, w = $('#landmark-size').valueAsNumber;
    const height = shape === 'sphere' ? w : $('#landmark-height').valueAsNumber, y = $('#landmark-elevation').valueAsNumber + height / 2;
    return { shape, y, pattern: $('#landmark-pattern').value, color: $('#landmark-color').value, pattern_color: $('#landmark-ink').value, pattern_size_cm: $('#landmark-pattern-size').valueAsNumber, pattern_tilt_deg: $('#landmark-tilt').valueAsNumber, w, d: ['sphere', 'cylinder'].includes(shape) ? w : $('#landmark-depth').valueAsNumber, h: shape === 'sphere' ? w : $('#landmark-height').valueAsNumber };
}
function updateLandmarkPreview() {
    const shape = $('#landmark-shape').value, pattern = $('#landmark-pattern').value;
    $('#landmark-size-label').textContent = ['sphere', 'cylinder'].includes(shape) ? 'Diameter · cm' : 'Width · cm';
    $('#landmark-height-row').hidden = shape === 'sphere';
    $('#landmark-depth-row').hidden = !['cube', 'diamond'].includes(shape);
    $('#landmark-tilt-row').hidden = pattern === 'solid';
    $('#landmark-tilt').disabled = pattern === 'solid';
    $('#landmark-ink-row').hidden = pattern === 'solid';
    $('#landmark-pattern-size-row').hidden = pattern === 'solid';
    $('#landmark-height').disabled = shape === 'sphere';
    $('#landmark-depth').disabled = !['cube', 'diamond'].includes(shape);
    $('#landmark-pattern-size').disabled = pattern === 'solid';
    try {
        const options = landmarkOptions();
        if (!Number.isFinite(options.y) || Math.abs(options.y) > 10000)
            throw new Error('Choose an elevation that keeps the centre between −10000 and 10000 cm.');
        for (const value of [options.w, options.d, options.h, options.pattern_size_cm])
            if (!Number.isFinite(value) || value < .1 || value > 10000)
                throw new Error('Enter dimensions between 0.1 and 10000 cm.');
        const o = object('landmark', { ...options, x: 0, s: 0, y: 0, angle: 0 });
        const span = Math.max(options.w, options.d, options.h), x = -span * 1.4, s = -span * 2.2, y = span * .9;
        const camera = { x, s, y, yaw: Math.atan2(-x, -s) * 180 / Math.PI, pitch: Math.atan2(y, Math.hypot(x, s)), fov: 40 };
        const image = landmarkPreview.render(solids({ objects: [o] }, { preview: true }), camera, 320, 200), canvas = $('#landmark-preview'), ctx = canvas.getContext('2d');
        const backdrop = ctx.createLinearGradient(0, 0, 0, 200);
        backdrop.addColorStop(0, '#1b3031');
        backdrop.addColorStop(1, '#536762');
        ctx.fillStyle = backdrop;
        ctx.fillRect(0, 0, 320, 200);
        const surface = document.createElement('canvas');
        surface.width = 320;
        surface.height = 200;
        surface.getContext('2d').putImageData(new ImageData(image.data, 320, 200), 0, 0);
        ctx.drawImage(surface, 0, 0);
        $('#landmark-message').textContent = '';
        $('#apply-landmark').disabled = false;
    }
    catch (e) {
        $('#landmark-message').textContent = e.message;
        $('#apply-landmark').disabled = true;
    }
}
function openLandmarkDialog(id = null) {
    landmarkEditingId = id;
    const source = id ? project.objects.find(o => o.id === id) : landmarkConfig;
    const o = { ...landmarkConfig, ...source, shape: source?.shape || 'sphere', pattern: source?.pattern || 'solid' };
    for (const [field, key] of [['shape', 'shape'], ['pattern', 'pattern'], ['color', 'color'], ['ink', 'pattern_color'], ['pattern-size', 'pattern_size_cm'], ['tilt', 'pattern_tilt_deg'], ['size', 'w'], ['height', 'h'], ['depth', 'd']])
        $('#landmark-' + field).value = o[key] ?? (key === 'pattern_tilt_deg' ? 0 : landmarkConfig[key]);
    $('#landmark-elevation').value = landmarkElevation({ ...o, kind: o.kind || 'landmark' });
    $('#apply-landmark').textContent = id ? 'Apply changes' : 'Place landmark';
    updateLandmarkPreview();
    $('#landmark-dialog').showModal();
}
$('#landmark-form').addEventListener('input', updateLandmarkPreview);
$('#landmark-form').addEventListener('submit', e => {
    e.preventDefault();
    if (!e.target.reportValidity())
        return;
    const options = landmarkOptions();
    if (landmarkEditingId) {
        if (!change(() => { const o = project.objects.find(o => o.id === landmarkEditingId); Object.assign(o, options, { kind: 'landmark' }); }))
            return;
    }
    else {
        landmarkConfig = { ...options };
        setTool('landmark');
    }
    $('#landmark-dialog').close();
    if (!landmarkEditingId)
        toast('Click the floor plan to place the landmark.');
});
$('#cancel-landmark').onclick = () => $('#landmark-dialog').close();
let panelWallId = null;
function panelOptions() { return { width_cm: $('#panel-width').valueAsNumber, gap_cm: $('#panel-gap').valueAsNumber, color: $('#panel-color').value, sides: $('#panel-sides').value }; }
function updatePanelSummary() {
    const wall = project.objects.find(o => o.id === panelWallId);
    try {
        const panels = wallPanels(wall, panelOptions(), 1000 - project.objects.length);
        $('#panels-summary').textContent = `${Math.max(wall.w, wall.d)} cm wall · ${panels.length} panels. The last panel is trimmed if needed.`;
        $('#create-panels').disabled = false;
    }
    catch (e) {
        $('#panels-summary').textContent = e.message;
        $('#create-panels').disabled = true;
    }
}
function openPanelDialog(id) { panelWallId = id; updatePanelSummary(); $('#panels-dialog').showModal(); }
$('#panels-form').addEventListener('input', updatePanelSummary);
$('#panels-form').addEventListener('submit', e => {
    e.preventDefault();
    if (!e.target.reportValidity())
        return;
    try {
        const wall = project.objects.find(o => o.id === panelWallId), panels = wallPanels(wall, panelOptions(), 1000 - project.objects.length);
        if (change(() => { project.objects.push(...panels); selectedIds = new Set(panels.map(o => o.id)); selected = panels.at(-1)?.id || null; setTool('select'); })) {
            $('#panels-dialog').close();
            toast(`${panels.length} panels added. The new panels are selected for editing.`);
        }
    }
    catch (error) {
        toast(error.message);
    }
});
$('#cancel-panels').onclick = () => $('#panels-dialog').close();
function duplicate() {
    const sources = selectedObjects();
    if (!sources.length)
        return;
    change(() => { const copies = sources.map(o => ({ ...clone(o), id: uid(), x: o.x + grid, s: o.s + grid, name: o.name.slice(0, 115) + ' copy' })); project.objects.push(...copies); selectedIds = new Set(copies.map(o => o.id)); selected = copies.at(-1).id; });
}
function copySelected() {
    const sources = selectedObjects();
    if (!sources.length)
        return;
    copiedObjects = clone(sources);
    toast(`${sources.length === 1 ? 'Object' : sources.length + ' objects'} copied. Point at the plan and press ⌘V / Ctrl+V to paste.`);
}
function pasteAtPointer() {
    if (!copiedObjects)
        return;
    const rect = plan.getBoundingClientRect();
    if (!pointerClient || pointerClient.clientX < rect.left || pointerClient.clientX >= rect.right || pointerClient.clientY < rect.top || pointerClient.clientY >= rect.bottom) {
        toast('Move the pointer over the floor plan to paste.');
        return;
    }
    const position = eventPoint(pointerClient), xs = copiedObjects.map(o => o.x), ss = copiedObjects.map(o => o.s);
    const dx = snap(position.x) - (Math.min(...xs) + Math.max(...xs)) / 2, ds = snap(position.s) - (Math.min(...ss) + Math.max(...ss)) / 2;
    change(() => { const copies = copiedObjects.map(o => ({ ...clone(o), id: uid(), x: o.x + dx, s: o.s + ds, name: o.name.slice(0, 115) + ' copy' })); project.objects.push(...copies); selectedIds = new Set(copies.map(o => o.id)); selected = copies.at(-1).id; setTool('select'); });
}
function distributeSelection() {
    try {
        const { positions, gapCm } = distributeEvenly(selectedObjects());
        if (change(() => { for (const position of positions)
            Object.assign(project.objects.find(o => o.id === position.id), position); }))
            toast(`Objects distributed with ${Number(gapCm.toFixed(2))} cm gaps.`);
    }
    catch (e) {
        toast(e.message);
    }
}
$('#distribute').onclick = distributeSelection;
function trackPointer(e) { pointerClient = { clientX: e.clientX, clientY: e.clientY }; }
document.addEventListener('pointermove', trackPointer);
document.addEventListener('pointerdown', trackPointer);
function remove() { if (selectedIds.size)
    change(() => { project.objects = project.objects.filter(o => !selectedIds.has(o.id)); selectOnly(null); }); }
plan.addEventListener('pointerdown', e => {
    if (e.button !== 0 && e.button !== 1 && !(e.ctrlKey && e.button === 2))
        return;
    e.preventDefault();
    plan.focus({ preventScroll: true });
    plan.setPointerCapture(e.pointerId);
    const p = eventPoint(e);
    hover = p;
    if (e.ctrlKey || e.button === 1) {
        drag = { pan: true, clientX: e.clientX, clientY: e.clientY, view: { ...view } };
        plan.style.cursor = 'grabbing';
        return;
    }
    if (tool === 'ruler') {
        const point = rulerEndpoint(e);
        if (!ruler || ruler.complete) {
            ruler = { from: point, to: point, complete: false };
            drag = { ruler: true, startClient: { x: e.clientX, y: e.clientY } };
        }
        else {
            ruler.to = point;
            ruler.complete = true;
            drag = null;
        }
        rulerHint();
        drawPlan();
        return;
    }
    const handle = !e.shiftKey ? resizeHandleHit(e) : null;
    if (handle) {
        drag = { resizeOriginal: clone(handle.object), corner: handle.index, before: clone(project) };
        plan.style.cursor = resizeCursor(handle.object, handle.index);
        return;
    }
    if ((tool === 'select' || tool === 'camera') && !e.shiftKey && cameraHit(e)) {
        drag = { start: p, cameraOriginal: clone(project.camera), before: clone(project) };
        selectOnly(null);
        cameraSelected = true;
        plan.style.cursor = 'move';
        refresh();
        return;
    }
    if (tool === 'select') {
        const hits = [...project.objects].reverse().filter(o => hit(o, p.x, p.s)), o = hits.find(o => o.kind !== 'floor') || hits[0];
        if (e.shiftKey && o && o.kind !== 'floor') {
            toggleSelection(o.id);
            drag = null;
            refresh();
            return;
        }
        if (o && !(o.kind === 'floor' && !selectedIds.has(o.id)) && !e.shiftKey) {
            if (!selectedIds.has(o.id))
                selectOnly(o.id);
            else
                selected = o.id;
            drag = { start: p, originals: clone(selectedObjects()), before: clone(project) };
            plan.style.cursor = 'move';
        }
        else {
            drag = { marquee: true, start: p, end: p, startClient: { x: e.clientX, y: e.clientY }, selectionBefore: [...selectedIds], clickedId: o?.id || null, additive: e.shiftKey };
            if (!e.shiftKey)
                selectOnly(null);
            plan.style.cursor = 'crosshair';
        }
        refresh();
    }
    else if (tool === 'camera') {
        change(() => { project.camera.x = snap(p.x); project.camera.s = snap(p.s); });
    }
    else if (tool === 'landmark' || tool === 'arch') {
        change(() => { const o = tool === 'landmark' ? object('landmark', { ...landmarkConfig, x: snap(p.x), s: snap(p.s) }) : object('arch', { x: snap(p.x), s: snap(p.s), w: 40, h: 40, color: '#b5bfba' }); project.objects.push(o); selectOnly(o.id); });
    }
    else
        drag = { start: { x: snap(p.x), s: snap(p.s) } };
});
plan.addEventListener('pointermove', e => {
    hover = eventPoint(e);
    $('#coordinates').textContent = `X ${Math.round(hover.x)} · Forward ${Math.round(hover.s)} cm`;
    if (!drag) {
        const handle = resizeHandleHit(e);
        plan.style.cursor = e.ctrlKey ? 'grab' : handle ? resizeCursor(handle.object, handle.index) : tool === 'select' ? (cameraHit(e) || project.objects.some(o => hit(o, hover.x, hover.s) && (o.kind !== 'floor' || selectedIds.has(o.id))) ? 'move' : 'crosshair') : 'crosshair';
    }
    if (tool === 'ruler' && ruler && !ruler.complete && !drag?.pan) {
        ruler.to = rulerEndpoint(e);
        rulerHint();
        drawPlan();
        return;
    }
    if (drag?.marquee) {
        drag.end = hover;
        selectedIds = new Set([...(drag.additive ? drag.selectionBefore : []), ...objectsInsideRect(project.objects, drag.start, drag.end)]);
        selected = [...selectedIds].at(-1) || null;
        drawPlan();
        return;
    }
    if (drag?.pan) {
        view.x = drag.view.x - (e.clientX - drag.clientX) / view.scale;
        view.s = drag.view.s + (e.clientY - drag.clientY) / view.scale;
    }
    else if (drag?.resizeOriginal) {
        const patch = resizeFromCorner(drag.resizeOriginal, drag.corner, hover, e.altKey ? 0.1 : grid);
        if (Math.abs(patch.x) <= 10000 && Math.abs(patch.s) <= 10000) {
            const o = project.objects.find(o => o.id === drag.resizeOriginal.id), elevation = isLandmark(o) ? landmarkElevation(drag.resizeOriginal) : null;
            Object.assign(o, patch);
            if (elevation !== null)
                Object.assign(o, landmarkAtElevation(o, elevation));
        }
    }
    else if (drag?.cameraOriginal) {
        project.camera.x = Math.max(-10000, Math.min(10000, snap(drag.cameraOriginal.x + hover.x - drag.start.x)));
        project.camera.s = Math.max(-10000, Math.min(10000, snap(drag.cameraOriginal.s + hover.s - drag.start.s)));
    }
    else if (drag?.originals) {
        const xs = drag.originals.map(o => o.x), ss = drag.originals.map(o => o.s);
        const dx = Math.max(-10000 - Math.min(...xs), Math.min(10000 - Math.max(...xs), snap(hover.x - drag.start.x)));
        const ds = Math.max(-10000 - Math.min(...ss), Math.min(10000 - Math.max(...ss), snap(hover.s - drag.start.s)));
        for (const original of drag.originals) {
            const o = project.objects.find(o => o.id === original.id);
            o.x = original.x + dx;
            o.s = original.s + ds;
        }
    }
    draw();
});
function finishDrag(e, cancel = false) {
    if (!drag)
        return;
    const current = drag;
    drag = null;
    if (current.ruler) {
        if (cancel)
            ruler = null;
        else if (Math.hypot(e.clientX - current.startClient.x, e.clientY - current.startClient.y) > 4) {
            ruler.to = rulerEndpoint(e);
            ruler.complete = true;
        }
        rulerHint();
        drawPlan();
        return;
    }
    if (current.pan) {
        plan.style.cursor = tool === 'select' ? 'default' : 'crosshair';
        draw();
        return;
    }
    if (current.marquee) {
        selectedIds = new Set(current.selectionBefore);
        reconcileSelection();
        if (!cancel) {
            const moved = Math.hypot(e.clientX - current.startClient.x, e.clientY - current.startClient.y) > 4;
            if (moved) {
                selectedIds = new Set([...(current.additive ? current.selectionBefore : []), ...objectsInsideRect(project.objects, current.start, eventPoint(e))]);
                selected = [...selectedIds].at(-1) || null;
            }
            else if (current.clickedId) {
                if (current.additive)
                    toggleSelection(current.clickedId);
                else
                    selectOnly(current.clickedId);
            }
            else if (!current.additive)
                selectOnly(null);
        }
        plan.style.cursor = 'default';
        refresh();
        return;
    }
    if (current.originals || current.cameraOriginal || current.resizeOriginal) {
        if (cancel)
            project = current.before;
        else if (JSON.stringify(current.before) !== JSON.stringify(project)) {
            history.push(current.before);
            if (history.length > 60)
                history.shift();
            future = [];
            persist();
        }
        refresh();
        return;
    }
    if (cancel) {
        draw();
        return;
    }
    const p = eventPoint(e), dx = snap(p.x) - current.start.x, ds = snap(p.s) - current.start.s;
    if (Math.hypot(dx, ds) < 1) {
        draw();
        toast('Drag to define the size.');
        return;
    }
    change(() => { const o = object(tool, { x: current.start.x + dx / 2, s: current.start.s + ds / 2, ...(tool === 'wall' ? { w: Math.hypot(dx, ds), d: 4, h: 30, angle: Math.atan2(ds, dx) * 180 / Math.PI } : { w: Math.max(grid, Math.abs(dx)), d: Math.max(grid, Math.abs(ds)), color: '#aab2ae' }) }); project.objects.push(o); selectOnly(o.id); });
}
// macOS maps Ctrl + click to a context menu; reserve it here for panning.
plan.addEventListener('contextmenu', e => { if (e.ctrlKey || drag?.pan)
    e.preventDefault(); });
plan.addEventListener('pointerup', e => finishDrag(e));
plan.addEventListener('pointercancel', e => finishDrag(e, true));
plan.addEventListener('lostpointercapture', e => finishDrag(e, true));
plan.addEventListener('wheel', e => { e.preventDefault(); const before = eventPoint(e); view.scale = Math.max(.03, Math.min(12, view.scale * Math.exp(-e.deltaY * .0015))); const after = eventPoint(e); view.x += before.x - after.x; view.s += before.s - after.s; draw(); }, { passive: false });
function toggleTools(open) { $('#tools-card').hidden = !open; $('#tools-toggle').setAttribute('aria-expanded', String(open)); }
$('#tools-toggle').onclick = () => toggleTools($('#tools-card').hidden);
for (const button of document.querySelectorAll('[data-tool]'))
    button.onclick = () => { toggleTools(false); if (button.dataset.tool === 'landmark')
        openLandmarkDialog();
    else {
        setTool(button.dataset.tool);
        plan.focus({ preventScroll: true });
    } };
document.addEventListener('pointerdown', e => { if (!e.target.closest('.tools-disclosure'))
    toggleTools(false); });
document.addEventListener('keydown', e => { if (e.key === 'Escape' && !$('#tools-card').hidden) {
    e.preventDefault();
    e.stopImmediatePropagation();
    toggleTools(false);
    $('#tools-toggle').focus();
} }, true);
$('#close-inspector').onclick = () => { selectOnly(null); setTool('select'); refresh(); };
installShortcutHelp($('#shortcut-overlay'));
$('#fit').onclick = fit;
$('#zoom-in').onclick = () => { view.scale = Math.min(12, view.scale * 1.25); draw(); };
$('#zoom-out').onclick = () => { view.scale = Math.max(.03, view.scale / 1.25); draw(); };
$('#grid').onchange = e => { grid = +e.target.value; draw(); };
$('#filter').oninput = refresh;
$('#undo').onclick = () => undo();
$('#redo').onclick = () => undo(true);
function setMode(value) { orbitDrag = null; mode = value; preview.style.cursor = mode === 'orbit' ? 'grab' : 'default'; preview.setAttribute('aria-label', mode === 'orbit' ? '3D overview: drag to rotate, scroll to zoom.' : '3D camera view: W/S or up/down to move, A/D or left/right to turn.'); $('#orbit').classList.toggle('active', mode === 'orbit'); $('#eye').classList.toggle('active', mode === 'eye'); draw(); }
preview.addEventListener('pointerdown', e => {
    if (mode === 'eye') {
        preview.focus({ preventScroll: true });
        return;
    }
    if (mode !== 'orbit' || e.button !== 0 || orbitDrag)
        return;
    e.preventDefault();
    preview.focus({ preventScroll: true });
    preview.setPointerCapture(e.pointerId);
    orbitDrag = { pointerId: e.pointerId, x: e.clientX, y: e.clientY, azimuth: orbit.azimuth, elevation: orbit.elevation };
    preview.style.cursor = 'grabbing';
});
preview.addEventListener('pointermove', e => {
    if (!orbitDrag || e.pointerId !== orbitDrag.pointerId)
        return;
    orbit.azimuth = (orbitDrag.azimuth - (e.clientX - orbitDrag.x) * .008) % (Math.PI * 2);
    // Keep away from the poles; allow looking at the scene from above or below.
    const pole = Math.PI / 2 - .01;
    orbit.elevation = Math.max(-pole, Math.min(pole, orbitDrag.elevation + (e.clientY - orbitDrag.y) * .008));
    drawPreview();
});
function finishOrbit(e) { if (!orbitDrag || e.pointerId !== orbitDrag.pointerId)
    return; orbitDrag = null; preview.style.cursor = mode === 'orbit' ? 'grab' : 'default'; }
preview.addEventListener('pointerup', finishOrbit);
preview.addEventListener('pointercancel', finishOrbit);
preview.addEventListener('lostpointercapture', finishOrbit);
preview.addEventListener('wheel', e => {
    if (mode !== 'orbit')
        return;
    e.preventDefault();
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? preview.clientHeight : 1;
    const delta = Math.max(-500, Math.min(500, e.deltaY * unit));
    orbit.distance = Math.max(.04, Math.min(8, orbit.distance * Math.exp(delta * .0015)));
    drawPreview();
}, { passive: false });
$('#orbit').onclick = () => setMode('orbit');
$('#eye').onclick = () => { setMode('eye'); preview.focus({ preventScroll: true }); };
$('#reset-view').onclick = () => { resetNavigation(); draw(); };
$('#walk').oninput = e => { const distance = +e.target.value, yaw = project.camera.yaw * Math.PI / 180; movePreviewTo({ x: project.camera.x + Math.sin(yaw) * distance, s: project.camera.s + Math.cos(yaw) * distance }); syncWalkControl(); setMode('eye'); };
$('#background-color').onchange = e => { const color = e.target.value; change(() => { project.background_color = color; }); };
$('#new').onclick = () => { if (!confirm('Create a project from this template? You can undo this change. Save a copy first to keep the current project.'))
    return; change(() => { project = template($('#template').value); selectOnly(null); resetNavigation(); }); fit(); };
function exportNavigationOptions() { return { mode: $('#export-mode').value, device: $('#wheel-device').value.trim(), axis: $('#wheel-axis').value.trim(), loop: $('#wheel-loop').checked, track_length_cm: $('#wheel-length').valueAsNumber, fade_frames: 0 }; }
function updateExportOptions() {
    syncRigLoad();
    const wheel = $('#export-mode').value === 'wheel', loop = $('#wheel-loop').checked;
    $('#wheel-export-options').hidden = !wheel;
    for (const id of ['wheel-device', 'wheel-axis', 'wheel-loop'])
        $('#' + id).disabled = !wheel;
    $('#wheel-length').disabled = !wheel || !loop;
    $('#wheel-fade').disabled = !wheel || !loop;
    try {
        validateNavigation(exportNavigationOptions());
        validateMovementStart(cameraMovement(project));
        $('#export-message').textContent = wheel ? 'The rig wheel drives the camera along its initial heading. Browser exploration stays temporary.' : 'Export geometry and the initial camera without navigation.';
        $('#download-scene').disabled = false;
    }
    catch (e) {
        $('#export-message').textContent = e.message;
        $('#download-scene').disabled = true;
    }
}
function openExportDialog() {
    const settings = project.navigation || { mode: 'wheel', device: 'wheel', axis: 'wheel', loop: true, track_length_cm: suggestTrackLength(project.objects, project.camera), fade_frames: 15 };
    $('#export-mode').value = settings.mode;
    $('#wheel-device').value = settings.device || 'wheel';
    $('#wheel-axis').value = settings.axis || 'wheel';
    $('#wheel-loop').checked = settings.loop !== false;
    $('#wheel-length').value = Number.isFinite(settings.track_length_cm) ? settings.track_length_cm : suggestTrackLength(project.objects, project.camera);
    $('#wheel-fade').value = settings.fade_frames ?? 15;
    if (!$('#rig-scene').value)
        $('#rig-scene').value = filename().slice(0, 64);
    updateExportOptions();
    $('#export-dialog').showModal();
}
function currentNavigation() { const input = exportNavigationOptions(); return input.mode === 'static' ? { mode: 'static' } : input; }
function currentRigName() { return $('#rig-project').value.trim() + '/' + $('#rig-scene').value.trim(); }
function rigSignature() { return JSON.stringify({ ...project, navigation: currentNavigation() }); }
function syncRigLoad() {
    $('#rig-load').disabled = rigBusy || !lastRigSave || lastRigSave.name !== currentRigName() || lastRigSave.signature !== rigSignature();
}
function prepareWorkshopScene() {
    const settings = currentNavigation();
    validateNavigation(settings);
    const scene = exportScene({ ...project, navigation: settings });
    if (JSON.stringify(project.navigation) !== JSON.stringify(settings) && !change(() => { project.navigation = settings; }))
        throw new Error('Could not save navigation settings.');
    return scene;
}
function rigStatus(message, error = false) { $('#rig-status').textContent = message; $('#rig-status').dataset.error = String(error); }
async function runRig(action) {
    if (rigBusy || !workshopRig)
        return;
    rigBusy = true;
    for (const id of ['rig-save', 'rig-connect'])
        $('#' + id).disabled = true;
    syncRigLoad();
    try {
        await action();
    }
    catch (e) {
        rigStatus(e.message || String(e), true);
    }
    finally {
        rigBusy = false;
        for (const id of ['rig-save', 'rig-connect'])
            $('#' + id).disabled = false;
        syncRigLoad();
    }
}
export function installWorkshopRig(api) {
    workshopRig = api;
    $('#rig-actions').hidden = false;
    $('#control-panel').hidden = false;
    $('#rig-project').addEventListener('input', syncRigLoad);
    $('#rig-scene').addEventListener('input', syncRigLoad);
    $('#rig-connect').onclick = () => runRig(async () => rigStatus(await api.connect()));
    $('#rig-save').onclick = () => runRig(async () => {
        if (!$('#export-form').reportValidity())
            return;
        const json = JSON.stringify(prepareWorkshopScene()), name = currentRigName(), signature = rigSignature();
        // A failed or timed-out write leaves its server-side outcome uncertain.
        // Require a successful save before enabling a subsequent display load.
        lastRigSave = null;
        syncRigLoad();
        rigStatus('Saving ' + name + '…');
        await api.save(name, json, $('#rig-overwrite').checked);
        lastRigSave = { name, json, signature };
        rigStatus('Saved ' + name + '. Ready to load on the display.');
    });
    $('#rig-load').onclick = () => runRig(async () => {
        if (!lastRigSave || lastRigSave.name !== currentRigName() || lastRigSave.signature !== rigSignature())
            throw new Error('The design or navigation has changed. Save it before loading.');
        rigStatus('Checking inputs and loading ' + lastRigSave.name + '…');
        await api.load(lastRigSave.name, lastRigSave.json);
        rigStatus('Loaded ' + lastRigSave.name + ' on the display.');
    });
    void runRig(async () => rigStatus(await api.connect()));
}
$('#export-form').addEventListener('input', updateExportOptions);
$('#export-form').addEventListener('submit', e => {
    e.preventDefault();
    if (!e.target.reportValidity())
        return;
    const input = exportNavigationOptions(), settings = input.mode === 'static' ? { mode: 'static' } : input;
    try {
        validateNavigation(settings);
        const scene = prepareWorkshopScene();
        download(scene, filename() + (settings.mode === 'wheel' ? '_wheel' : '') + '.config.json');
        $('#export-dialog').close();
        const approximate = project.objects.some(o => o.kind === 'landmark' && o.pattern !== 'solid');
        toast((settings.mode === 'wheel' ? 'Wheel scene exported. Save project to keep its navigation settings.' : 'Static scene exported.') + (approximate ? ' Landmark patterns use a coarse approximation in vstimd.' : ''));
    }
    catch (error) {
        toast(error.message);
    }
});
$('#cancel-export').onclick = () => $('#export-dialog').close();
function filename() { return project.name.normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^a-zA-Z0-9_-]+/g, '_').replace(/^[-_]+/, '').slice(0, 50) || 'maze'; }
function download(data, name) { const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2) + '\n'], { type: 'application/json' })), link = document.createElement('a'); link.href = url; link.download = name; link.click(); setTimeout(() => URL.revokeObjectURL(url), 10000); }
$('#save').onclick = () => { download(project, filename() + '.maze.json'); toast('Project downloaded. You can open it again in the editor.'); };
$('#export').onclick = openExportDialog;
$('#load').onclick = () => $('#file').click();
$('#file').onchange = async (e) => { const file = e.target.files[0]; if (!file)
    return; try {
    if (file.size > 2000000)
        throw new Error('Project exceeds the 2 MB limit.');
    const incoming = validate(JSON.parse(await file.text()));
    change(() => { project = incoming; selectOnly(null); resetNavigation(); });
    fit();
    toast('Project opened.');
}
catch (err) {
    toast('Could not open: ' + err.message);
}
finally {
    e.target.value = '';
} };
function handleCameraKey(e) {
    if (e.target !== preview || mode !== 'eye' || e.metaKey || e.ctrlKey || e.altKey)
        return false;
    const key = e.key.toLowerCase();
    if (!['w', 'a', 's', 'd', 'arrowup', 'arrowdown', 'arrowleft', 'arrowright'].includes(key))
        return false;
    e.preventDefault();
    if (previewReturn.active)
        return true;
    if (key === 'a' || key === 'arrowleft')
        turn -= 5;
    if (key === 'd' || key === 'arrowright')
        turn += 5;
    const forward = ['w', 'arrowup'].includes(key) ? 5 : ['s', 'arrowdown'].includes(key) ? -5 : 0;
    const heading = (project.camera.yaw + turn) * Math.PI / 180;
    const from = previewPosition(), to = { x: Math.max(-10000, Math.min(10000, from.x + Math.sin(heading) * forward)), s: Math.max(-10000, Math.min(10000, from.s + Math.cos(heading) * forward)) };
    movePreviewTo(to);
    syncWalkControl();
    draw();
    return true;
}
document.addEventListener('keydown', e => { if (['INPUT', 'SELECT', 'TEXTAREA'].includes(e.target.tagName) || e.target.isContentEditable || $('#panels-dialog').open || $('#landmark-dialog').open || $('#export-dialog').open)
    return; if (e.key === 'Escape' && drag) {
    e.preventDefault();
    finishDrag(e, true);
    return;
} if ((e.metaKey || e.ctrlKey) && !e.altKey) {
    const key = e.key.toLowerCase();
    if (key === 'c' && selectedIds.size) {
        e.preventDefault();
        if (!e.repeat)
            copySelected();
        return;
    }
    if (key === 'v' && copiedObjects) {
        e.preventDefault();
        if (!e.repeat)
            pasteAtPointer();
        return;
    }
} if (e.code === 'Space' && e.target === plan)
    e.preventDefault(); if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') {
    e.preventDefault();
    undo(e.shiftKey);
    return;
} if (handleCameraKey(e))
    return; if (e.key === 'Delete' || e.key === 'Backspace') {
    e.preventDefault();
    remove();
} if (e.key === 'Escape') {
    selectOnly(null);
    setTool('select');
    refresh();
} if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'd') {
    e.preventDefault();
    duplicate();
    return;
} if (!e.metaKey && !e.ctrlKey) {
    const tools = { v: 'select', w: 'wall', f: 'floor', l: 'landmark', a: 'arch', c: 'camera', r: 'ruler' };
    if (e.key.toLowerCase() === 'l')
        openLandmarkDialog();
    else if (tools[e.key.toLowerCase()])
        setTool(tools[e.key.toLowerCase()]);
} });
preview.style.cursor = 'grab';
installPreviewResizer({ handle: $('#preview-resizer'), preview, plan: $('.plan-wrap'), workspace: $('.workspace'), onResize: draw });
persist();
refresh();
fit();
setTool('select');
