/// Application-level key actions, shared between the DRM and winit backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKey {
    Escape,
    F1,
    F2,
    F3,
    D,
    /// Ctrl+Alt+Fn — forward to the kernel as a VT switch.
    SwitchVt(u16),
}
