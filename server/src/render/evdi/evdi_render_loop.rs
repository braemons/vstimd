//! evdi backend: renders through the normal Vulkan scene pipeline into a
//! headless swapchain image (see `evdi_init.rs`), reads that image back to
//! the CPU, and presents it on the evdi KMS output (`evdi_kms.rs`).
//!
//! No vblank clock and no VT handling — neither applies here.
//! `render_frame()` is called with `screen_clock = Some(Instant::now())` so
//! it never tries to touch `VK_KHR_present_wait` machinery that has no
//! meaning on a headless swapchain.
//!
//! Unlike the DRM backend this owns no VT. The kernel console framebuffer
//! lives on the *real* display controller (vc4 on a Pi 5), a different DRM
//! device from evdi's, so switching VTs cannot change what is on the
//! DisplayLink panel — vstimd holds evdi's master and nothing else draws to
//! it. Ctrl+Alt+Fn is therefore left to the kernel rather than forwarded,
//! and the only VT interaction is suppressing echo on the *active* VT so
//! keystrokes meant for the overlay don't queue up in a getty.
//!
//! Keyboard handling and the egui overlay are otherwise exactly the DRM
//! backend's, via the shared helpers in `render::frame_loop`.

use ash::vk;

use crate::log_buffer::LogBuffer;
use crate::render::backend::BackendData;
use crate::input::console_input::{InputState, active_vt};
use crate::render::frame_loop::{self, KeyOutcome};
use crate::render::render_frame;
use crate::render::render_state::RenderState;
use crate::system_info::{ClockSource, SystemInfo};
use crate::render::vk::VkContext;
use crate::render::{
    ReadbackTarget, RenderTarget, SceneRenderer, StimulusDisplayInfo, TextRenderer, UiRenderer,
};
use crate::timing::FrameTiming;

use super::evdi_detect::find_connected_evdi;
use super::evdi_init;
use super::evdi_kms::EvdiOutput;

pub struct EvdiBackend {
    data: BackendData,
    log_buffer: LogBuffer,
}

impl EvdiBackend {
    pub fn new(data: BackendData, log_buffer: LogBuffer) -> Self {
        Self { data, log_buffer }
    }

    pub fn run(self, on_ready: impl FnOnce()) {
        let BackendData {
            scene,
            vtl,
            host_info,
            overlay_scale,
            ..
        } = self.data;

        let node = find_connected_evdi().unwrap_or_else(|| {
            eprintln!("vstimd: no connected evdi (DisplayLink) output found");
            std::process::exit(1);
        });
        let mut output = EvdiOutput::new(node).unwrap_or_else(|e| {
            eprintln!("vstimd: failed to set up evdi KMS output: {e}");
            std::process::exit(1);
        });

        let ctx = evdi_init::init(output.width, output.height);

        let storage_dir = scene.read().expect("scene lock poisoned").runtime.storage_dir.clone();
        let scene_renderer = SceneRenderer::new(&ctx, scene);
        let text = TextRenderer::new(&ctx);
        let ui = UiRenderer::new(&ctx, storage_dir, self.log_buffer, overlay_scale);

        // Guard the VT that is currently active: unlike the DRM backend we
        // never activate one of our own, so that is where stray keystrokes
        // would otherwise land.
        let mut input = InputState::new(active_vt().unwrap_or(1));

        let display_info = StimulusDisplayInfo {
            width_px: output.width,
            height_px: output.height,
            refresh_hz: output.mode.vrefresh() as f64,
            mode_index: None,
        };
        let system_info = SystemInfo {
            host: host_info,
            gpu_name: String::new(),
            backend: RenderTarget::Evdi,
            supports_wireframe: ctx.supports_wireframe,
            // evdi has no vblank clock at all (see evdi_kms.rs) — there is
            // nothing more accurate than a GPU-completion timestamp here.
            clock_source: ClockSource::GpuCompletion,
        };

        let mut rs = RenderState {
            scene_renderer,
            text,
            ui: Some(ui),
            // DisplayLink holds the mode's rate on average but delivers in
            // bursts, so loss is measured cumulatively rather than per
            // interval (see `Pacing::AveragedRate`) — a real shortfall is
            // still reported, ordinary jitter is not.
            timing: FrameTiming::new_rate_averaged(display_info.refresh_hz),
            system_info,
            display_info,
            ctx,
        };

        // Dropped before `rs` (declared after it) so the readback's Vulkan
        // objects are destroyed while `rs.ctx.device` is still alive.
        let readback = Readback::new(&rs.ctx, output.pitch(), output.height);

        log::info!(
            "vstimd: evdi backend running, {}×{}",
            output.width,
            output.height
        );
        on_ready();

        loop {
            if crate::process::shutdown::is_requested() {
                break;
            }

            // Poll keyboard input (non-blocking libinput drain). A VT switch
            // is a no-op here: we hold no VT, and evdi is a different DRM
            // device from the console framebuffer, so the kernel handles
            // Ctrl+Alt+Fn on its own.
            let (app_keys, nav_events) = input.poll();
            for key in app_keys {
                if let KeyOutcome::SwitchVt(n) = frame_loop::apply_app_key(key, &mut rs) {
                    log::debug!("vstimd: ignoring VT switch to tty{n} — evdi output is unaffected");
                }
            }
            let egui_raw_input = frame_loop::overlay_raw_input(&rs, nav_events);

            frame_loop::advance_frame(vtl.as_ref(), &rs.scene_renderer.scene);

            let t_render = std::time::Instant::now();
            let (tick, _platform_output) = render_frame(
                &mut rs,
                Some(std::time::Instant::now()),
                egui_raw_input,
                vtl.as_deref(),
                Some(&readback.target),
            );
            // The copy above ran inside render_frame's own command buffer,
            // before it presented — render_frame already waited for it to
            // land, so the staging buffer is ready to read now.
            let render_us = t_render.elapsed().as_micros() as u32;
            // Headless swapchains don't hit ERROR_OUT_OF_DATE_KHR (no real
            // presentation engine, no resize) — this is defensive, not
            // expected to trigger.
            let Some(tick) = tick else { continue };

            let t_readback = std::time::Instant::now();
            let frame_bytes = readback.frame();
            let readback_us = t_readback.elapsed().as_micros() as u32;

            // Collect the *previous* frame's flip event here, not right after
            // submitting it: everything above (render + readback of this
            // frame) then overlaps with DisplayLink draining the last one.
            let t_flip_wait = std::time::Instant::now();
            if let Err(e) = output.wait_flip() {
                log::error!("vstimd: evdi flip wait failed: {e} — stopping");
                break;
            }
            let flip_wait_us = t_flip_wait.elapsed().as_micros() as u32;

            let t_submit = std::time::Instant::now();
            if let Err(e) = output.submit(frame_bytes) {
                log::error!("vstimd: evdi present failed: {e} — stopping");
                break;
            }
            let submit_us = t_submit.elapsed().as_micros() as u32;

            // evdi has no vblank to miss, so `render_frame`'s drop counter is
            // meaningless here (see FrameTiming::new_unpaced below) and stays
            // silent. Log the stage breakdown periodically instead — it is
            // the only visibility into where a frame's time actually goes.
            if tick.frame.is_multiple_of(600) {
                log::debug!(
                    "vstimd: evdi frame {} [render={}µs readback={}µs flip_wait={}µs present={}µs]",
                    tick.frame,
                    render_us,
                    readback_us,
                    flip_wait_us,
                    submit_us
                );
            }
        }
    }
}

// ── Readback ─────────────────────────────────────────────────────────────────

/// A persistent host-visible staging buffer that `render_frame` copies the
/// rendered swapchain image into (see `ReadbackTarget`), before it presents.
/// Sized with `evdi`'s row pitch (not a tightly-packed `width * 4`) so the
/// result can be handed to `EvdiOutput::present` unmodified.
struct Readback {
    device: ash::Device,
    memory: vk::DeviceMemory,
    mapped: *mut u8,
    size: usize,
    target: ReadbackTarget,
}

impl Readback {
    fn new(ctx: &VkContext, pitch: usize, height: u32) -> Self {
        let device = ctx.device.clone();
        let size = pitch * height as usize;

        let buffer = unsafe {
            device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size as vk::DeviceSize)
                        .usage(vk::BufferUsageFlags::TRANSFER_DST)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .expect("failed to create evdi readback buffer")
        };
        let reqs = unsafe { device.get_buffer_memory_requirements(buffer) };
        let mem_props =
            unsafe { ctx.instance.get_physical_device_memory_properties(ctx.physical_device) };
        let mem_type = (0..mem_props.memory_type_count)
            .find(|&i| {
                (reqs.memory_type_bits & (1 << i)) != 0
                    && mem_props.memory_types[i as usize].property_flags.contains(
                        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
            })
            .expect("no HOST_VISIBLE|HOST_COHERENT memory for evdi readback buffer");
        let memory = unsafe {
            device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(reqs.size)
                        .memory_type_index(mem_type),
                    None,
                )
                .expect("failed to allocate evdi readback memory")
        };
        unsafe {
            device.bind_buffer_memory(buffer, memory, 0).expect("bind readback memory");
        }
        let mapped = unsafe {
            device
                .map_memory(memory, 0, size as vk::DeviceSize, vk::MemoryMapFlags::empty())
                .expect("failed to map evdi readback memory") as *mut u8
        };

        Self {
            device,
            memory,
            mapped,
            size,
            target: ReadbackTarget {
                buffer,
                row_length_texels: (pitch / 4) as u32,
            },
        }
    }

    /// A view of the staging buffer as of the last `render_frame` call that
    /// was given `Some(&self.target)`. `render_frame` waits for its copy to
    /// land on the GPU before returning, so this is always current.
    fn frame(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.mapped, self.size) }
    }
}

impl Drop for Readback {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.target.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
