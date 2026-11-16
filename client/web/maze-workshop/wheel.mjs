// Scene-side navigation only. Device calibration and shared memory belong to the rig.
export function validateNavigation(nav) {
    if (!nav || !['static', 'wheel'].includes(nav.mode))
        throw new Error('Choose static or wheel navigation.');
    if (nav.mode === 'static')
        return;
    for (const key of ['device', 'axis'])
        if (typeof nav[key] !== 'string' || !/^[A-Za-z0-9_.-]{1,64}$/.test(nav[key]))
            throw new Error(`Invalid wheel ${key} name.`);
    if (typeof nav.loop !== 'boolean')
        throw new Error('Choose whether to return at the end of the track.');
    if (nav.loop) {
        if (!Number.isFinite(nav.track_length_cm) || nav.track_length_cm < .1 || nav.track_length_cm > 100000)
            throw new Error('Track length must be between 0.1 and 100000 cm.');
        if (!Number.isInteger(nav.fade_frames) || nav.fade_frames < 0 || nav.fade_frames > 600)
            throw new Error('Fade must be a whole number between 0 and 600 frames.');
    }
}
export function wheelAnimation(nav) {
    if (nav === undefined || nav.mode === 'static')
        return null;
    validateNavigation(nav);
    return { name: 'wheel_navigation', state: 'Armed', target: { kind: 'Camera' }, start_action: 0, start_action_trigger_line: null, final_action: 0, final_action_trigger_line: null, start_trigger: null, animation: { LinearNav3D: { speed_cm_per_s: 0, wrap_period_cm: null, source: { device: nav.device, axis: nav.axis }, track: nav.loop ? { length_cm: nav.track_length_cm, fade_frames: 0 } : null } } };
}
export function suggestTrackLength(objects, camera) {
    const heading = camera.yaw * Math.PI / 180, forwardX = Math.sin(heading), forwardS = Math.cos(heading);
    let max = 0;
    for (const o of objects.filter(o => o.kind === 'floor')) {
        const a = o.angle * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
        for (const u of [-o.w / 2, o.w / 2])
            for (const v of [-o.d / 2, o.d / 2]) {
                const x = o.x + c * u - s * v, along = o.s + s * u + c * v;
                max = Math.max(max, (x - camera.x) * forwardX + (along - camera.s) * forwardS);
            }
    }
    return max > 0 ? Math.max(.1, Math.min(100000, Number(max.toFixed(1)))) : 400;
}
export function previewTrackLength(project) {
    if (project.navigation?.loop === false)
        return null;
    return project.navigation?.track_length_cm ?? suggestTrackLength(project.objects, project.camera);
}
export function previewTrackProgress(camera, point) {
    const yaw = camera.yaw * Math.PI / 180;
    return (point.x - camera.x) * Math.sin(yaw) + (point.s - camera.s) * Math.cos(yaw);
}
