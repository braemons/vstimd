//! One module per overlay group.
//!
//! Each `*_panel` function draws only the *body* of its group — the chrome
//! (the `Panel::left`, its background colour, the title bar and the close
//! button) is applied uniformly by `overlay::build_overlay_ui`, so a panel
//! never has to know where it sits or how it is framed.

pub(super) mod animations_panel;
pub(super) mod benchmarks_panel;
pub(super) mod config_panel;
pub(super) mod log_panel;
pub(super) mod stimuli_panel;
pub(super) mod system_panel;
pub(super) mod vtl_panel;
