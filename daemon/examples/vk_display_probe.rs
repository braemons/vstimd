//! Diagnostic: what does vkGetPhysicalDeviceDisplayPropertiesKHR actually
//! report for each VkDisplayKHR -- how many displays, display_name,
//! physical_resolution -- and does physical_resolution already give the
//! native resolution without needing DRM-level ModeTypeFlags::PREFERRED?

use ash::vk;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let entry = unsafe { ash::Entry::load().expect("load vulkan") };
    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
    let exts = [ash::khr::surface::NAME.as_ptr(), ash::khr::display::NAME.as_ptr()];
    let info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&exts);
    let instance = unsafe { entry.create_instance(&info, None).expect("create instance") };
    let display_loader = ash::khr::display::Instance::new(&entry, &instance);

    let physical_devices = unsafe { instance.enumerate_physical_devices().expect("enum pd") };
    for pd in physical_devices {
        let props = unsafe { instance.get_physical_device_properties(pd) };
        let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) };
        println!("physical device: {name:?} type={:?}", props.device_type);

        let display_props = unsafe {
            display_loader.get_physical_device_display_properties(pd)
        };
        match display_props {
            Ok(list) => {
                println!("  {} VkDisplayKHR reported:", list.len());
                for (i, d) in list.iter().enumerate() {
                    let dn = if d.display_name.is_null() {
                        "<null>".to_string()
                    } else {
                        unsafe { std::ffi::CStr::from_ptr(d.display_name) }
                            .to_string_lossy()
                            .into_owned()
                    };
                    println!(
                        "    [{i}] name={dn:?} physical_resolution={}x{} physical_dimensions_mm={}x{}",
                        d.physical_resolution.width,
                        d.physical_resolution.height,
                        d.physical_dimensions.width,
                        d.physical_dimensions.height,
                    );

                    let modes = unsafe {
                        display_loader.get_display_mode_properties(pd, d.display)
                    };
                    if let Ok(modes) = modes {
                        for (mi, m) in modes.iter().enumerate() {
                            println!(
                                "        mode[{mi}] {}x{} @ {}.{:03} Hz",
                                m.parameters.visible_region.width,
                                m.parameters.visible_region.height,
                                m.parameters.refresh_rate / 1000,
                                m.parameters.refresh_rate % 1000,
                            );
                        }
                    }
                }
            }
            Err(e) => println!("  get_physical_device_display_properties failed: {e:?}"),
        }
    }

    unsafe { instance.destroy_instance(None) };
}
