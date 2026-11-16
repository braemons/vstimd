// Temporary preview coordinates; these helpers never edit scene.camera.
export function minimapTransform(bounds, width, height, padding = 8) {
    const spanX = Math.max(20, bounds.maxX - bounds.minX), spanS = Math.max(20, bounds.maxS - bounds.minS);
    return { x: (bounds.minX + bounds.maxX) / 2, s: (bounds.minS + bounds.maxS) / 2, scale: Math.max(.00001, Math.min(Math.max(1, width - 2 * padding) / spanS, Math.max(1, height - 2 * padding) / spanX)), width, height };
}
export function worldToMinimap(point, map) { return { x: map.width / 2 + (point.s - map.s) * map.scale, y: map.height / 2 + (point.x - map.x) * map.scale }; }
export function minimapToWorld(point, map) { return { x: map.x + (point.y - map.height / 2) / map.scale, s: map.s + (point.x - map.width / 2) / map.scale }; }
export function previewOffsets(camera, point) { return { walkX: point.x - camera.x, walk: point.s - camera.s }; }
