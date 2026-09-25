use ash::vk;

use crate::render::StimulusDisplayInfo;
use crate::render::backend::DisplayModePref;
use crate::render::vk::{VkContext, build_context, create_vk_instance};

/// Initialise Vulkan for bare-metal display via `VK_KHR_display`.
///
/// Enumerates connected displays, picks a mode, creates the display surface,
/// and returns a fully-initialised `VkContext` plus the `VkDisplayKHR` handle
/// (needed for `VK_EXT_display_control` vblank fences).
pub fn init(display_pref: DisplayModePref) -> (VkContext, StimulusDisplayInfo, vk::DisplayKHR) {
    // VK_EXT_display_surface_counter is an instance extension required by
    // VK_EXT_display_control (device).  Enable it when available.
    let available_inst_exts: std::collections::HashSet<String> = unsafe {
        ash::Entry::load()
            .expect("failed to load libvulkan.so")
            .enumerate_instance_extension_properties(None)
            .unwrap_or_default()
            .into_iter()
            .map(|e| {
                std::ffi::CStr::from_ptr(e.extension_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    };
    let use_display_surface_counter =
        available_inst_exts.contains("VK_EXT_display_surface_counter");

    // VK_KHR_display lets Vulkan enumerate and drive displays directly
    // without requiring a compositor.
    let mut base_exts = vec![
        ash::khr::surface::NAME.as_ptr(),
        ash::khr::display::NAME.as_ptr(),
    ];
    if use_display_surface_counter {
        base_exts.push(ash::ext::display_surface_counter::NAME.as_ptr());
    }

    let (entry, instance, debug_utils_enabled) = create_vk_instance(&base_exts);

    let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
    let display_loader = ash::khr::display::Instance::new(&entry, &instance);

    // Pick a physical device that has a graphics queue.
    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("no Vulkan physical devices")
    };
    let (physical_device, _) = physical_devices
        .iter()
        .find_map(|&pd| find_graphics_queue(&instance, pd).map(|qf| (pd, qf)))
        .expect("no Vulkan device with a graphics queue");

    // Enumerate connected displays; render to display[0] for now.
    let all_display_props = unsafe {
        display_loader
            .get_physical_device_display_properties(physical_device)
            .expect("vkGetPhysicalDeviceDisplayPropertiesKHR failed")
    };
    assert!(
        !all_display_props.is_empty(),
        "no Vulkan displays found — is the display connected and the driver loaded?"
    );
    let vk_display = all_display_props[0].display;

    let mode_props = unsafe {
        display_loader
            .get_display_mode_properties(physical_device, vk_display)
            .expect("failed to get display mode properties")
    };
    assert!(
        !mode_props.is_empty(),
        "no display modes reported for display — check driver and display connection"
    );
    let (mode_index, chosen) = pick_mode(&mode_props, display_pref);
    let display_mode = chosen.display_mode;
    let width = chosen.parameters.visible_region.width;
    let height = chosen.parameters.visible_region.height;

    let plane_props = unsafe {
        display_loader
            .get_physical_device_display_plane_properties(physical_device)
            .expect("failed to get display plane properties")
    };
    let plane_index = (0..plane_props.len() as u32)
        .find(|&i| unsafe {
            display_loader
                .get_display_plane_supported_displays(physical_device, i)
                .map(|ds| ds.contains(&vk_display))
                .unwrap_or(false)
        })
        .unwrap_or(0);

    let surface = unsafe {
        display_loader
            .create_display_plane_surface(
                &vk::DisplaySurfaceCreateInfoKHR::default()
                    .display_mode(display_mode)
                    .plane_index(plane_index)
                    .plane_stack_index(0)
                    .transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                    .global_alpha(1.0)
                    .alpha_mode(vk::DisplayPlaneAlphaFlagsKHR::OPAQUE)
                    .image_extent(vk::Extent2D { width, height }),
                None,
            )
            .expect("failed to create Vulkan display surface")
    };

    let extent = vk::Extent2D { width, height };
    let ctx = build_context(
        entry,
        instance,
        surface,
        surface_loader,
        extent,
        debug_utils_enabled,
        use_display_surface_counter,
    );

    let refresh_hz = chosen.parameters.refresh_rate as f64 / 1000.0;
    log::info!("vstimd: display {}×{}  {:.3} Hz", width, height, refresh_hz);

    (
        ctx,
        StimulusDisplayInfo {
            width_px: width,
            height_px: height,
            refresh_hz,
            mode_index: Some(mode_index),
        },
        vk_display,
    )
}

/// Does `m` satisfy every `Some` field in `pref`? Fields left `None` are not
/// filtered on. `refresh_hz` matches within 1 Hz to tolerate EDID rates like
/// 59.94/60.007 that aren't exact integers.
fn mode_matches(m: &vk::DisplayModePropertiesKHR, pref: DisplayModePref) -> bool {
    if let Some(w) = pref.width
        && m.parameters.visible_region.width != w
    {
        return false;
    }
    if let Some(h) = pref.height
        && m.parameters.visible_region.height != h
    {
        return false;
    }
    if let Some(hz) = pref.refresh_hz {
        let reported_hz = m.parameters.refresh_rate as f64 / 1000.0;
        if (reported_hz - hz).abs() >= 1.0 {
            return false;
        }
    }
    true
}

/// Highest refresh rate first, then highest resolution as a tie-break.
/// `Iterator::max_by_key` returns the *last* of equally-maximal elements, and
/// drivers commonly report same-refresh modes in descending-resolution order
/// — without the resolution tie-break this silently picked the smallest mode.
fn mode_rank(m: &vk::DisplayModePropertiesKHR) -> (u32, u64) {
    let w = m.parameters.visible_region.width as u64;
    let h = m.parameters.visible_region.height as u64;
    (m.parameters.refresh_rate, w * h)
}

fn pick_mode(
    modes: &[vk::DisplayModePropertiesKHR],
    pref: DisplayModePref,
) -> (usize, vk::DisplayModePropertiesKHR) {
    log::info!("vstimd: available display modes:");
    for (i, m) in modes.iter().enumerate() {
        let w = m.parameters.visible_region.width;
        let h = m.parameters.visible_region.height;
        let hz = m.parameters.refresh_rate;
        log::info!("  [{}] {}×{}  {}.{:03} Hz", i, w, h, hz / 1000, hz % 1000);
    }

    // Allow override via VSTIMD_DISPLAY_MODE=<index>. Takes priority over the
    // rig-config preference — it's for interactive debugging.
    if let Ok(s) = std::env::var("VSTIMD_DISPLAY_MODE") {
        match s.trim().parse::<usize>() {
            Ok(i) if i < modes.len() => {
                let m = &modes[i];
                let w = m.parameters.visible_region.width;
                let h = m.parameters.visible_region.height;
                let hz = m.parameters.refresh_rate;
                log::info!(
                    "vstimd: using display mode {} (VSTIMD_DISPLAY_MODE): {}×{}  {}.{:03} Hz",
                    i,
                    w,
                    h,
                    hz / 1000,
                    hz % 1000
                );
                return (i, modes[i]);
            }
            Ok(i) => log::warn!(
                "vstimd: VSTIMD_DISPLAY_MODE={i} out of range (0..{}), using auto-select",
                modes.len()
            ),
            Err(_) => {
                log::warn!("vstimd: VSTIMD_DISPLAY_MODE={s:?} is not a number, using auto-select")
            }
        }
    }

    // rig-config [display] preference: filter to matching modes and pick the
    // best among them. Falls through to plain auto-select if nothing matches.
    if pref.width.is_some() || pref.height.is_some() || pref.refresh_hz.is_some() {
        let best = modes
            .iter()
            .enumerate()
            .filter(|(_, m)| mode_matches(m, pref))
            .max_by_key(|(_, m)| mode_rank(m));
        match best {
            Some((idx, m)) => {
                let w = m.parameters.visible_region.width;
                let h = m.parameters.visible_region.height;
                let hz = m.parameters.refresh_rate;
                log::info!(
                    "vstimd: using rig-config preferred display mode [{idx}] {w}×{h}  {}.{:03} Hz",
                    hz / 1000,
                    hz % 1000
                );
                return (idx, *m);
            }
            None => log::warn!(
                "vstimd: rig-config display preference ({}×{}@{}Hz) matches no reported mode \
                 (see the list above) — falling back to auto-select",
                pref.width.map_or("*".to_string(), |w| w.to_string()),
                pref.height.map_or("*".to_string(), |h| h.to_string()),
                pref.refresh_hz.map_or("*".to_string(), |hz| hz.to_string()),
            ),
        }
    }

    // Auto-select: highest refresh rate, then highest resolution.
    // modes is guaranteed non-empty by the assert at the call site.
    let (best_idx, best) = modes
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| mode_rank(m))
        .expect("mode list is empty");
    let w = best.parameters.visible_region.width;
    let h = best.parameters.visible_region.height;
    let hz = best.parameters.refresh_rate;
    log::info!(
        "vstimd: auto-selected display mode {}×{}  {}.{:03} Hz",
        w,
        h,
        hz / 1000,
        hz % 1000
    );
    (best_idx, *best)
}

fn find_graphics_queue(instance: &ash::Instance, pd: vk::PhysicalDevice) -> Option<u32> {
    let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
    families.iter().enumerate().find_map(|(i, p)| {
        p.queue_flags
            .contains(vk::QueueFlags::GRAPHICS)
            .then_some(i as u32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic mode list — no GPU/display needed, `pick_mode` only
    /// reads `visible_region`/`refresh_rate`. Mirrors what `VK_KHR_display`
    /// actually reported in the field (see the bug report this covers):
    /// same refresh rate throughout, descending resolution.
    fn mode(width: u32, height: u32, refresh_mhz: u32) -> vk::DisplayModePropertiesKHR {
        vk::DisplayModePropertiesKHR {
            parameters: vk::DisplayModeParametersKHR {
                visible_region: vk::Extent2D { width, height },
                refresh_rate: refresh_mhz,
            },
            ..Default::default()
        }
    }

    fn field_modes() -> Vec<vk::DisplayModePropertiesKHR> {
        vec![
            mode(3840, 2160, 60007),
            mode(1920, 1200, 60007),
            mode(1920, 1080, 60007),
            mode(1600, 1200, 60007),
            mode(1680, 1050, 60007),
            mode(1280, 1024, 60007),
            mode(1440, 900, 60007),
            mode(1280, 800, 60007),
            mode(1280, 720, 60007),
            mode(1024, 768, 60007),
            mode(800, 600, 60007),
            mode(640, 480, 60007),
        ]
    }

    #[test]
    fn auto_select_prefers_highest_resolution_on_tied_refresh_rate() {
        // Regression test: `max_by_key` returns the *last* of tied elements,
        // and this exact mode list (reported in descending-resolution order,
        // identical refresh rates) used to auto-select 640×480 — the worst
        // mode on the list — instead of the 3840×2160 at index 0.
        let modes = field_modes();
        let (idx, chosen) = pick_mode(&modes, DisplayModePref::default());
        assert_eq!(idx, 0);
        assert_eq!(chosen.parameters.visible_region.width, 3840);
        assert_eq!(chosen.parameters.visible_region.height, 2160);
    }

    #[test]
    fn rig_config_preference_selects_matching_mode() {
        let modes = field_modes();
        let pref = DisplayModePref { width: Some(1920), height: Some(1080), refresh_hz: None };
        let (idx, chosen) = pick_mode(&modes, pref);
        assert_eq!(idx, 2);
        assert_eq!(chosen.parameters.visible_region.width, 1920);
        assert_eq!(chosen.parameters.visible_region.height, 1080);
    }

    #[test]
    fn rig_config_refresh_hz_matches_within_tolerance() {
        // EDID rates are rarely exact integers (60.007 Hz here); the config
        // value 60.0 must still match.
        let modes = field_modes();
        let pref = DisplayModePref { width: Some(1920), height: Some(1080), refresh_hz: Some(60.0) };
        let (idx, _) = pick_mode(&modes, pref);
        assert_eq!(idx, 2);
    }

    #[test]
    fn rig_config_preference_falls_back_to_auto_select_when_no_match() {
        let modes = field_modes();
        let pref = DisplayModePref { width: Some(2560), height: Some(1440), refresh_hz: None };
        let (idx, chosen) = pick_mode(&modes, pref);
        // Falls back to the same auto-select result as the empty-pref case.
        assert_eq!(idx, 0);
        assert_eq!(chosen.parameters.visible_region.width, 3840);
    }

    #[test]
    fn mode_rank_orders_by_refresh_then_resolution() {
        let low_res_high_hz = mode(1280, 720, 120000);
        let high_res_low_hz = mode(3840, 2160, 60000);
        assert!(mode_rank(&low_res_high_hz) > mode_rank(&high_res_low_hz));

        let a = mode(1920, 1080, 60000);
        let b = mode(3840, 2160, 60000);
        assert!(mode_rank(&b) > mode_rank(&a));
    }
}
