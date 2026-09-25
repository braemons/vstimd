use ash::vk;

/// Load Vulkan and create an instance requesting `extensions`.
///
/// In debug builds, `VK_EXT_debug_utils` is appended and silently dropped if
/// the driver rejects it (e.g. when RenderDoc is not injected).
///
/// `VSTIMD_VK_VALIDATION=1` additionally enables `VK_LAYER_KHRONOS_validation`
/// (in any build), which reports to stdout. It is off by default: the layer costs
/// frame time, and a rig must never depend on it being installed.
///
/// Returns `(Entry, Instance, debug_utils_enabled)`.
pub fn create_vk_instance(
    extensions: &[*const std::ffi::c_char],
) -> (ash::Entry, ash::Instance, bool) {
    let entry = unsafe { ash::Entry::load().expect("failed to load libvulkan.so") };
    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
    let layers = validation_layers(&entry);

    #[cfg(debug_assertions)]
    let (instance, debug_utils_enabled) = {
        let mut with_debug: Vec<*const std::ffi::c_char> = extensions.to_vec();
        with_debug.push(ash::ext::debug_utils::NAME.as_ptr());
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&with_debug);
        match unsafe { entry.create_instance(&info, None) } {
            Ok(inst) => (inst, true),
            Err(vk::Result::ERROR_EXTENSION_NOT_PRESENT) => {
                log::debug!(
                    "vstimd: VK_EXT_debug_utils not accepted at vkCreateInstance — disabling"
                );
                let info_bare = vk::InstanceCreateInfo::default()
                    .application_info(&app_info)
                    .enabled_layer_names(&layers)
                    .enabled_extension_names(extensions);
                let inst = unsafe {
                    entry
                        .create_instance(&info_bare, None)
                        .expect("failed to create Vulkan instance")
                };
                (inst, false)
            }
            Err(e) => panic!("failed to create Vulkan instance: {e}"),
        }
    };
    #[cfg(not(debug_assertions))]
    let (instance, debug_utils_enabled) = {
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(extensions);
        let inst = unsafe {
            entry
                .create_instance(&info, None)
                .expect("failed to create Vulkan instance")
        };
        (inst, false)
    };

    (entry, instance, debug_utils_enabled)
}

const VALIDATION_LAYER: &std::ffi::CStr = c"VK_LAYER_KHRONOS_validation";

/// The validation layer, when `VSTIMD_VK_VALIDATION` asks for it and it is
/// installed. Missing is a warning, not an error — it is a debugging aid.
fn validation_layers(entry: &ash::Entry) -> Vec<*const std::ffi::c_char> {
    if std::env::var_os("VSTIMD_VK_VALIDATION").is_none_or(|v| v.is_empty() || v == "0") {
        return Vec::new();
    }
    let available = unsafe { entry.enumerate_instance_layer_properties() }.unwrap_or_default();
    let installed = available
        .iter()
        .any(|l| l.layer_name_as_c_str().is_ok_and(|n| n == VALIDATION_LAYER));
    if installed {
        log::info!("vstimd: Vulkan validation layer enabled");
        vec![VALIDATION_LAYER.as_ptr()]
    } else {
        log::warn!(
            "vstimd: VSTIMD_VK_VALIDATION is set but VK_LAYER_KHRONOS_validation is not \
             installed (Debian/Ubuntu: vulkan-validationlayers)"
        );
        Vec::new()
    }
}
