const FRAME_HISTORY: usize = 120;

pub struct FrameSummary {
    pub fps: f64,
    pub mean_ms: f64,
    pub std_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub drop_count: u64,
    pub frame_index: u64,
}

pub struct FrameStats {
    frame_index: u64,
    last_present: Option<std::time::Instant>,
    durations_ns: [u64; FRAME_HISTORY],
    ring_head: usize,
    valid_count: usize,
    drop_count: u64,
    expected_frame_ns: u64,
}

impl FrameStats {
    pub fn new(target_hz: f64) -> Self {
        Self {
            frame_index: 0,
            last_present: None,
            durations_ns: [0; FRAME_HISTORY],
            ring_head: 0,
            valid_count: 0,
            drop_count: 0,
            expected_frame_ns: (1_000_000_000.0 / target_hz) as u64,
        }
    }

    pub fn on_present(&mut self) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last_present {
            let dur_ns = now.duration_since(last).as_nanos() as u64;
            let threshold = self.expected_frame_ns * 3 / 2;
            if dur_ns > threshold && self.expected_frame_ns > 0 {
                self.drop_count += (dur_ns / self.expected_frame_ns).saturating_sub(1);
            }
            self.durations_ns[self.ring_head] = dur_ns;
            self.ring_head = (self.ring_head + 1) % FRAME_HISTORY;
            if self.valid_count < FRAME_HISTORY {
                self.valid_count += 1;
            }
        }
        self.last_present = Some(now);
        self.frame_index += 1;
    }

    pub fn summary(&self) -> FrameSummary {
        let durations = &self.durations_ns[..self.valid_count.min(FRAME_HISTORY)];
        if durations.is_empty() {
            return FrameSummary {
                fps: 0.0, mean_ms: 0.0, std_ms: 0.0,
                min_ms: 0.0, max_ms: 0.0,
                drop_count: self.drop_count,
                frame_index: self.frame_index,
            };
        }
        let n = durations.len() as f64;
        let mean_ns = durations.iter().sum::<u64>() as f64 / n;
        let var_ns = durations.iter().map(|&d| { let x = d as f64 - mean_ns; x * x }).sum::<f64>() / n;
        FrameSummary {
            fps:        if mean_ns > 0.0 { 1_000_000_000.0 / mean_ns } else { 0.0 },
            mean_ms:    mean_ns / 1_000_000.0,
            std_ms:     var_ns.sqrt() / 1_000_000.0,
            min_ms:     *durations.iter().min().unwrap() as f64 / 1_000_000.0,
            max_ms:     *durations.iter().max().unwrap() as f64 / 1_000_000.0,
            drop_count: self.drop_count,
            frame_index: self.frame_index,
        }
    }
}
