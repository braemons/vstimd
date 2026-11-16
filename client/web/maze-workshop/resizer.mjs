// View preference only: never changes the project, camera or exported scene.
export function installPreviewResizer({ handle, preview, plan, workspace, onResize }) {
    const storageKey = 'braemons.maze-editor.preview-height';
    const minimum = 120;
    let drag = null, frame = null;
    const redraw = () => { if (frame === null)
        frame = requestAnimationFrame(() => { frame = null; onResize(); }); };
    const maximum = () => {
        const style = getComputedStyle(workspace), padding = parseFloat(style.paddingTop) + parseFloat(style.paddingBottom);
        const reserved = [...workspace.children].filter(node => node !== plan && node !== preview.parentElement).reduce((sum, node) => {
            const css = getComputedStyle(node);
            return sum + node.getBoundingClientRect().height + (parseFloat(css.marginTop) || 0) + (parseFloat(css.marginBottom) || 0);
        }, 0);
        const planMinimum = parseFloat(getComputedStyle(plan).minHeight) || 180;
        return Math.max(minimum, Math.floor(workspace.clientHeight - padding - reserved - planMinimum));
    };
    const clamp = height => Math.max(minimum, Math.min(maximum(), height));
    const describe = () => {
        const height = Math.round(preview.getBoundingClientRect().height);
        handle.setAttribute('aria-valuemin', String(minimum));
        handle.setAttribute('aria-valuemax', String(maximum()));
        handle.setAttribute('aria-valuenow', String(height));
        handle.setAttribute('aria-valuetext', `${height} pixels high`);
    };
    const setHeight = height => { preview.style.height = `${clamp(height)}px`; describe(); redraw(); };
    const save = () => { try {
        localStorage.setItem(storageKey, String(Math.round(preview.getBoundingClientRect().height)));
    }
    catch { /* optional preference */ } };
    const reset = () => { preview.style.removeProperty('height'); try {
        localStorage.removeItem(storageKey);
    }
    catch { /* optional preference */ } describe(); redraw(); };
    try {
        const saved = Number(localStorage.getItem(storageKey));
        if (Number.isFinite(saved) && saved > 0)
            setHeight(saved);
    }
    catch { /* file-mode storage may be unavailable */ }
    const finish = cancel => {
        if (!drag)
            return;
        const previous = drag;
        drag = null;
        if (cancel)
            setHeight(previous.height);
        else
            save();
        document.documentElement.classList.remove('resizing-preview');
        if (handle.hasPointerCapture(previous.pointerId))
            handle.releasePointerCapture(previous.pointerId);
    };
    handle.addEventListener('pointerdown', event => {
        if (event.button !== 0 || drag)
            return;
        event.preventDefault();
        handle.focus({ preventScroll: true });
        drag = { pointerId: event.pointerId, y: event.clientY, height: preview.getBoundingClientRect().height };
        handle.setPointerCapture(event.pointerId);
        document.documentElement.classList.add('resizing-preview');
    });
    handle.addEventListener('pointermove', event => { if (drag && event.pointerId === drag.pointerId) {
        event.preventDefault();
        setHeight(drag.height + drag.y - event.clientY);
    } });
    handle.addEventListener('pointerup', event => { if (drag && event.pointerId === drag.pointerId)
        finish(false); });
    handle.addEventListener('pointercancel', () => finish(true));
    handle.addEventListener('lostpointercapture', () => finish(true));
    window.addEventListener('blur', () => finish(true));
    document.addEventListener('keydown', event => { if (event.key === 'Escape' && drag) {
        event.preventDefault();
        event.stopImmediatePropagation();
        finish(true);
    } }, true);
    handle.addEventListener('dblclick', reset);
    handle.addEventListener('keydown', event => {
        if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key))
            return;
        event.preventDefault();
        event.stopPropagation();
        const height = preview.getBoundingClientRect().height, step = event.shiftKey ? 50 : 10;
        setHeight(event.key === 'Home' ? minimum : event.key === 'End' ? maximum() : height + (event.key === 'ArrowUp' ? step : -step));
        save();
    });
    new ResizeObserver(() => {
        const height = preview.getBoundingClientRect().height;
        if (preview.style.height && Math.abs(clamp(height) - height) > .5)
            setHeight(height);
        describe();
        redraw();
    }).observe(workspace);
    describe();
}
