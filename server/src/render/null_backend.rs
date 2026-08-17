use crate::vtl_state::VtlOutputs;
use crate::render::backend::BackendData;

pub struct NullBackend {
    data: BackendData,
}

impl NullBackend {
    pub fn new(data: BackendData) -> Self {
        Self { data }
    }

    pub fn run(self, on_ready: impl FnOnce()) {
        let BackendData { scene, vtl, .. } = self.data;

        log::info!("vstimd: null renderer — ZMQ server + animation loop running, no display");
        on_ready();

        let frame_period = {
            let s = scene.read().unwrap();
            std::time::Duration::from_secs_f32(1.0 / s.runtime.frame_rate)
        };
        loop {
            if crate::process::shutdown::is_requested() {
                break;
            }
            let t0 = std::time::Instant::now();
            // commit_staged writes the previous frame's outputs to shm; output_edges
            // then reads them back so animation-to-animation chaining works headlessly.
            let (input_edges, output_edges, mut levels, mut pulses) = vtl
                .as_ref()
                .and_then(|v| v.lock().ok().map(|mut g| {
                    g.commit_staged();
                    let input_edges = g.poll();
                    let output_edges = g.output_edges();
                    (input_edges, output_edges, g.staged, g.pulses)
                }))
                .unwrap_or_default();
            {
                let mut s = scene.write().unwrap();
                if s.runtime.pending_flip {
                    s.apply_flip();
                }
                s.runtime.frame_count += 1;
                let _ = s.runtime.frame_notifier.send(s.runtime.frame_count);
                s.advance_animations(
                    &input_edges,
                    &output_edges,
                    &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses },
                );
            }
            if let Some(v) = vtl.as_ref() {
                v.lock().unwrap().store_frame_outputs(levels, pulses);
            }
            if let Some(remaining) = frame_period.checked_sub(t0.elapsed()) {
                std::thread::sleep(remaining);
            }
        }
    }
}
