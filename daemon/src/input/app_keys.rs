use crate::render::overlay_ui::OverlayGroup;

/// Application-level key actions, shared between the DRM and winit backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKey {
    /// Close the open dialog, else hide the overlay. Never quits.
    Escape,
    /// Toggle the whole overlay on/off (backtick).
    ToggleOverlay,
    /// Show (make visible + focus) one overlay group (F1–F7).
    ShowGroup(OverlayGroup),
    /// Hide one overlay group (Shift+F1–F7).
    HideGroup(OverlayGroup),
    /// Spawn demo stimuli (only acted on when the overlay is hidden).
    D,
    /// Save the next rendered frame as a PNG (F12 / PrintScreen).
    ///
    /// Belongs here rather than in a backend because a rig running on bare
    /// DRM has no window manager to ask for a screenshot, and it is exactly
    /// that frame — the one on the panel — worth capturing.
    Screenshot,
    /// Ctrl+Alt+Fn — forward to the kernel as a VT switch.
    SwitchVt(u16),
    /// Ctrl+Q — quit the process (DRM mode has no window manager to send a
    /// close request, so this is the only in-session quit hotkey).
    Quit,
}
