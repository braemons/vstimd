//! The one-pass scene snapshot the web control surface polls.

use crate::proto;
use crate::scene::SceneState;
use crate::vtl_state::VtlState;

impl SceneState {

    /// Build a complete scene snapshot for the web control surface in a single
    /// pass. Takes `&self` so the web snapshot pump can hold only a read lock
    /// (minimal contention with the render thread's write lock). All stimuli are
    /// serialized in draw order; no per-stimulus dispatch.
    pub fn build_snapshot(&self, vtl: Option<&VtlState>) -> proto::SceneSnapshot {
        let server_info = match self.cmd_query_server_info().body {
            Some(proto::response::Body::ServerInfo(s)) => Some(s),
            _ => None,
        };
        let stimuli: Vec<proto::QueryStimulusResponse> = self
            .config
            .stimuli
            .iter()
            .map(|(h, e)| self.query_stimulus_response(*h, e))
            .collect();
        let animations = match self.cmd_list_animations().body {
            Some(proto::response::Body::AnimationList(a)) => Some(a),
            _ => None,
        };
        let vtl_lines = match self.cmd_list_virtual_trigger_lines(vtl).body {
            Some(proto::response::Body::VirtualTriggerLineList(l)) => Some(l),
            _ => None,
        };
        let command_log = self
            .runtime
            .command_log
            .iter()
            .map(|c| proto::CommandLogEntry {
                handle: c.handle,
                summary: c.summary.clone(),
                code: c.response,
                server_time_ns: (c.elapsed_ms * 1_000_000.0) as u64,
            })
            .collect();
        proto::SceneSnapshot {
            server_info,
            stimuli,
            animations,
            vtl_lines,
            vtl_state: None,
            command_log,
            frame_count: self.runtime.frame_count,
            server_time_ns: self.runtime.server_start.elapsed().as_nanos() as u64,
        }
    }
}
