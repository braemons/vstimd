//! Sanity check: does VK_EXT_headless_surface actually give us a working
//! VkContext (device + swapchain) via the existing build_context() shared
//! with the DRM/Winit backends? No evdi/KMS involved yet.

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let ctx = vstimd::render::evdi::init_headless_vk(1366, 768);
    println!(
        "headless VkContext ok: format={:?} extent={:?} swapchain_images={} frames_in_flight={}",
        ctx.format,
        ctx.extent,
        ctx.swapchain_images.len(),
        ctx.frames.len()
    );
}
