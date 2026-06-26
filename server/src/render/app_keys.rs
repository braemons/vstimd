use crate::render::overlay_ui::OverlayGroup;

/// Application-level key actions, shared between the DRM and winit backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKey {
    /// Close the open dialog, else hide the overlay. Never quits.
    Escape,
    /// Toggle the whole overlay on/off (backtick).
    ToggleOverlay,
    /// Toggle + focus one overlay group (F1–F7).
    SelectGroup(OverlayGroup),
    /// Spawn demo stimuli (only acted on when the overlay is hidden).
    D,
    /// Ctrl+Alt+Fn — forward to the kernel as a VT switch.
    SwitchVt(u16),
}
