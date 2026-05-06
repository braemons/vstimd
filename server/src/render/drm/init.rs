use ash::vk;

use crate::render::vk::{VkContext, build_context};

pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub refresh_mhz: u32,
}

/// Initialise Vulkan for bare-metal display via `VK_KHR_display`.
///
/// Enumerates connected displays, prompts the user to pick a mode, creates the
/// display surface, and returns a fully initialised `VkContext`.
pub fn init() -> (VkContext, DisplayInfo) {
    let entry = unsafe { ash::Entry::load().expect("failed to load libvulkan.so") };

    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);

    // Probe for VK_EXT_acquire_drm_display (implies VK_EXT_direct_mode_display).
    // Together these let us formally acquire the display from the kernel before
    // rendering and release it cleanly on exit via vkReleaseDisplayEXT.
    // Available on desktop NVIDIA with nvidia-drm.modeset=1; absent on Tegra
    // where the GPU and display controller are separate DRM nodes.
    let available_inst_exts: std::collections::HashSet<String> = unsafe {
        entry
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
    let has_inst_ext = |name: &std::ffi::CStr| {
        available_inst_exts.contains(name.to_str().unwrap_or(""))
    };
    let use_acquire_drm = has_inst_ext(ash::ext::acquire_drm_display::NAME);
    // acquire_drm requires direct_mode_display per spec; check independently
    // as a fallback for drivers that expose release without acquire.
    let use_direct_mode = use_acquire_drm
        || has_inst_ext(ash::ext::direct_mode_display::NAME);

    // VK_KHR_display lets Vulkan enumerate and drive displays directly.
    // On Tegra, VK_EXT_acquire_drm_display does not work because the Vulkan
    // device (nvgpu) and the display controller are separate hardware nodes.
    let mut instance_exts = vec![
        ash::khr::surface::NAME.as_ptr(),
        ash::khr::display::NAME.as_ptr(),
    ];
    if use_direct_mode {
        instance_exts.push(ash::ext::direct_mode_display::NAME.as_ptr());
    }
    if use_acquire_drm {
        instance_exts.push(ash::ext::acquire_drm_display::NAME.as_ptr());
    }

    let instance_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&instance_exts);
    let instance = unsafe {
        entry
            .create_instance(&instance_info, None)
            .expect("failed to create Vulkan instance")
    };

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

    // Enumerate all connected displays.  We render to display[0] for now;
    // the full list is kept so every display can be acquired and released —
    // NVIDIA disables the CRTC of any untracked display when it takes DRM
    // master, causing "no signal" on secondary monitors at exit.
    // Future: iterate the list to create one surface+swapchain per display
    // for mirrored output.
    let all_display_props = unsafe {
        display_loader
            .get_physical_device_display_properties(physical_device)
            .expect("vkGetPhysicalDeviceDisplayPropertiesKHR failed")
    };
    assert!(
        !all_display_props.is_empty(),
        "no Vulkan displays found — is the display connected and the driver loaded?"
    );
    let all_displays: Vec<vk::DisplayKHR> =
        all_display_props.iter().map(|p| p.display).collect();

    // Render target: first display (TODO: make configurable / iterate all).
    let vk_display = all_displays[0];

    let mode_props = unsafe {
        display_loader
            .get_display_mode_properties(physical_device, vk_display)
            .expect("failed to get display mode properties")
    };
    let chosen = pick_mode(&mode_props);
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

    // Formally acquire ALL connected displays before creating the surface.
    // Acquiring only the render target is not enough: NVIDIA disables the
    // CRTC of every display it doesn't track, leaving non-rendered monitors
    // with no signal.  We release all of them on exit so fbcon can reclaim
    // every CRTC.
    let (drm_fd, acquired_displays): (Option<std::fs::File>, Vec<vk::DisplayKHR>) =
        if use_acquire_drm {
            let acquire_loader =
                ash::ext::acquire_drm_display::Instance::new(&entry, &instance);
            acquire_all_displays(&acquire_loader, physical_device, &all_displays)
        } else {
            (None, vec![])
        };

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
    let mut ctx = build_context(entry, instance, surface, surface_loader, extent);

    if use_direct_mode {
        let release_loader =
            ash::ext::direct_mode_display::Instance::new(&ctx.entry, &ctx.instance);
        ctx.release_display = Some((release_loader, acquired_displays, drm_fd));
    } else {
        eprintln!("wonderlamp: VK_EXT_direct_mode_display not available — display release on exit may require a VT switch");
    }

    eprintln!(
        "wonderlamp: display {}×{}  {}.{:03} Hz",
        width,
        height,
        chosen.parameters.refresh_rate / 1000,
        chosen.parameters.refresh_rate % 1000,
    );

    (
        ctx,
        DisplayInfo {
            width,
            height,
            refresh_mhz: chosen.parameters.refresh_rate,
        },
    )
}

fn pick_mode(modes: &[vk::DisplayModePropertiesKHR]) -> vk::DisplayModePropertiesKHR {
    eprintln!("\nAvailable display modes:");
    for (i, m) in modes.iter().enumerate() {
        let w = m.parameters.visible_region.width;
        let h = m.parameters.visible_region.height;
        let hz = m.parameters.refresh_rate;
        eprintln!("  [{i}] {w}×{h}  {}.{:03} Hz", hz / 1000, hz % 1000);
    }
    loop {
        eprint!("Select mode [0]: ");
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .expect("failed to read stdin");
        let t = line.trim();
        if t.is_empty() {
            return modes[0];
        }
        match t.parse::<usize>() {
            Ok(i) if i < modes.len() => return modes[i],
            _ => eprintln!("  Enter 0–{}", modes.len() - 1),
        }
    }
}

/// Acquire every display in `displays` via VK_EXT_acquire_drm_display.
///
/// Tries /dev/dri/cardN nodes until one accepts the first display, then uses
/// the same fd to acquire the rest (all displays on one GPU share a DRM node).
/// Returns the open fd (must outlive all release calls) and the list of
/// successfully acquired display handles.
fn acquire_all_displays(
    loader: &ash::ext::acquire_drm_display::Instance,
    physical_device: vk::PhysicalDevice,
    displays: &[vk::DisplayKHR],
) -> (Option<std::fs::File>, Vec<vk::DisplayKHR>) {
    use std::os::unix::io::AsRawFd as _;

    // Find the DRM card that owns these displays.
    let Some(drm_file) = (0..8u32).find_map(|n| {
        let path = format!("/dev/dri/card{n}");
        let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(&path) else {
            return None;
        };
        let result = unsafe {
            (loader.fp().acquire_drm_display_ext)(physical_device, file.as_raw_fd(), displays[0])
        };
        eprintln!("wonderlamp: vkAcquireDrmDisplayEXT({path}, display[0]) -> {result:?}");
        (result == vk::Result::SUCCESS).then_some(file)
    }) else {
        eprintln!("wonderlamp: could not acquire any display — vkReleaseDisplayEXT will likely fail");
        return (None, vec![]);
    };

    // Acquire remaining displays on the same fd.
    let mut acquired = vec![displays[0]];
    for &display in &displays[1..] {
        let result = unsafe {
            (loader.fp().acquire_drm_display_ext)(physical_device, drm_file.as_raw_fd(), display)
        };
        eprintln!("wonderlamp: vkAcquireDrmDisplayEXT(display[{}]) -> {result:?}", acquired.len());
        if result == vk::Result::SUCCESS {
            acquired.push(display);
        }
    }

    (Some(drm_file), acquired)
}

fn find_graphics_queue(instance: &ash::Instance, pd: vk::PhysicalDevice) -> Option<u32> {
    let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
    families.iter().enumerate().find_map(|(i, p)| {
        p.queue_flags
            .contains(vk::QueueFlags::GRAPHICS)
            .then_some(i as u32)
    })
}
