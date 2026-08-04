const FRAME_HISTORY_SIZE: usize = 120;

/// Per-render-loop timing bookkeeping: aggregated frame statistics, last
/// per-phase breakdown, and the swapchain frame index.
pub struct FrameTiming {
    pub stats: FrameStats,
    pub last_phases: FramePhases,
    /// Swapchain slot index (cycles 0..swapchain_len); distinct from the global
    /// frame counter inside `FrameStats`.
    pub frame_index: usize,
}

impl FrameTiming {
    pub fn new(refresh_hz: f64) -> Self {
        Self {
            stats: FrameStats::new(refresh_hz),
            last_phases: FramePhases::default(),
            frame_index: 0,
        }
    }

    /// For a backend paced by a downstream consumer that holds the target
    /// rate on average but delivers irregularly. See [`Pacing::AveragedRate`].
    pub fn new_rate_averaged(refresh_hz: f64) -> Self {
        Self {
            stats: FrameStats::new_rate_averaged(refresh_hz),
            last_phases: FramePhases::default(),
            frame_index: 0,
        }
    }
}

/// Timing information for one successfully presented frame.
///
/// Returned from `render_frame` on every successful present.
/// The sequence of `FrameTick` values **is** the time axis of the server:
/// each tick maps a vblank serial number to the wall-clock time at which
/// it fired.
///
/// # Scheduling
/// - Use `frame` to express stimulus schedules in vblanks:
///   "start at frame N, show for M frames". Integer arithmetic, exact.
/// - Use `vblank_time` for experiment logging: record it as the stimulus
///   onset time in your data file.
/// - Check `dropped_frames` each tick; a non-zero value means the GPU
///   missed a deadline and the previous stimulus was shown for an extra
///   vblank. Flag the trial if timing precision matters.
#[derive(Debug, Clone)]
pub struct FrameTick {
    /// Present-ID assigned to this frame (1-based, resets after swapchain
    /// recreation). Monotonically increasing within a session.
    /// Use as the frame-number axis for scheduling stimuli.
    pub frame: u64,
    /// `Instant` captured immediately after `vkWaitForPresentKHR` returned,
    /// i.e. the best available proxy for the vblank that confirmed the
    /// *previous* frame on screen. On the first frame (no prior present)
    /// this is the time `render_frame` was entered.
    pub vblank_time: std::time::Instant,
    /// Extra vblanks elapsed beyond the expected one since the previous tick.
    /// 0 = on time.  1 = one dropped frame (GPU overran its budget once).
    pub dropped_frames: u32,
    /// Per-phase breakdown for profiling (see `FramePhases`).
    pub phases: FramePhases,
    /// Index into `VkContext::swapchain_images` that was just rendered into
    /// and presented. The DRM/Winit backends have no use for this (the real
    /// presentation engine already consumed it); the evdi backend uses it to
    /// read the image back for KMS presentation on a non-WSI display.
    pub image_index: u32,
}

pub struct FrameSummary {
    pub fps: f64,
    pub mean_ms: f64,
    pub std_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub drop_count: u64,
    pub frame_index: u64,
}

/// Wall-clock time (µs) spent in each phase of `render_frame`.
/// Accumulated per frame and available for logging or overlay display.
#[derive(Debug, Clone, Copy, Default)]
pub struct FramePhases {
    pub tessellate_us: u32, // scene write-lock: tess + GPU upload
    pub fence_us: u32,      // wait_for_fences
    pub acquire_us: u32,    // acquire_next_image
    pub record_us: u32,     // command buffer record
    pub submit_us: u32,     // queue_submit + queue_present
}

/// How a backend's frames are paced, which decides what "dropped" can even
/// mean for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pacing {
    /// Frames are locked to a hardware vblank. Every interval should be one
    /// period, so an interval longer than that identifies a *specific*
    /// missed vblank — the stimulus was on screen an extra refresh, which is
    /// exactly what an experiment needs flagged.
    Vblank,
    /// Frames are paced by a downstream consumer that averages the target
    /// rate but delivers in bursts (evdi: DisplayLinkManager drains frames
    /// over USB at the mode's rate, but individual intervals measured on a
    /// Pi 5 range 8–34 ms around a 16.6 ms mean).
    ///
    /// Testing each interval against the period reports a drop on every long
    /// one even though nothing was lost, so loss is instead measured as a
    /// cumulative deficit: frames that *should* have been presented by now
    /// at the target rate, minus those actually presented. Jitter cancels
    /// out; a genuine sustained shortfall accumulates and is still reported.
    AveragedRate,
}

pub struct FrameStats {
    frame_index: u64,
    last_present: Option<std::time::Instant>,
    durations_ns: [u64; FRAME_HISTORY_SIZE],
    ring_head: usize,
    valid_count: usize,
    drop_count: u64,
    expected_frame_ns: u64,
    pacing: Pacing,
    /// `AveragedRate` only: when the first frame was presented, and the
    /// deficit already accounted for, so each shortfall is reported once.
    first_present: Option<std::time::Instant>,
    reported_deficit: u64,
}

impl FrameStats {
    pub fn new(target_hz: f64) -> Self {
        Self::with_pacing(target_hz, Pacing::Vblank)
    }

    /// See [`Pacing::AveragedRate`].
    pub fn new_rate_averaged(target_hz: f64) -> Self {
        Self::with_pacing(target_hz, Pacing::AveragedRate)
    }

    fn with_pacing(target_hz: f64, pacing: Pacing) -> Self {
        let expected_frame_ns = if target_hz.is_finite() && target_hz > 0.0 {
            (1_000_000_000.0 / target_hz) as u64
        } else {
            0
        };
        Self {
            frame_index: 0,
            last_present: None,
            durations_ns: [0; FRAME_HISTORY_SIZE],
            ring_head: 0,
            valid_count: 0,
            drop_count: 0,
            expected_frame_ns,
            pacing,
            first_present: None,
            reported_deficit: 0,
        }
    }

    /// Record a presented frame using the vblank timestamp captured
    /// immediately after `vkWaitForPresentKHR` returned.
    ///
    /// Using the actual vblank time rather than `Instant::now()` gives
    /// accurate inter-frame intervals independent of render duration.
    ///
    /// Returns the number of frames dropped since the previous call
    /// (0 = on time). The same value is included in the `FrameTick`
    /// returned from `render_frame`.
    /// Returns true while still in the warmup window (first few frames).
    /// Callers should suppress drop warnings during this period.
    pub fn is_warming_up(&self) -> bool {
        self.frame_index < 5
    }

    pub fn on_present(&mut self, vblank_time: std::time::Instant) -> u32 {
        let dropped = if let Some(last) = self.last_present {
            let dur_ns = vblank_time.duration_since(last).as_nanos() as u64;
            let d = match self.pacing {
                Pacing::Vblank => self.count_missed_vblanks(dur_ns),
                Pacing::AveragedRate => self.count_rate_deficit(vblank_time),
            };
            self.drop_count += d as u64;
            self.durations_ns[self.ring_head] = dur_ns;
            self.ring_head = (self.ring_head + 1) % FRAME_HISTORY_SIZE;
            if self.valid_count < FRAME_HISTORY_SIZE {
                self.valid_count += 1;
            }
            d
        } else {
            self.first_present = Some(vblank_time);
            0
        };
        self.last_present = Some(vblank_time);
        self.frame_index += 1;
        dropped
    }

    /// One interval against one period — see [`Pacing::Vblank`].
    fn count_missed_vblanks(&self, dur_ns: u64) -> u32 {
        // 5/4 threshold: trigger if the interval exceeds 1.25× the expected period.
        // Using round-to-nearest division avoids the truncation bug where
        // 2 × period computes as 1.999× and floors to 1 → sub(1) = 0.
        let threshold = self.expected_frame_ns.saturating_mul(5) / 4;
        if self.expected_frame_ns == 0 || dur_ns <= threshold {
            return 0;
        }
        ((dur_ns + self.expected_frame_ns / 2) / self.expected_frame_ns).saturating_sub(1) as u32
    }

    /// Frames owed against frames delivered — see [`Pacing::AveragedRate`].
    ///
    /// Floor division on the elapsed time deliberately under-counts by up to
    /// one frame, so ordinary jitter can never manufacture a drop; only a
    /// shortfall that persists long enough to cost a whole frame is reported.
    fn count_rate_deficit(&mut self, now: std::time::Instant) -> u32 {
        let (Some(start), true) = (self.first_present, self.expected_frame_ns > 0) else {
            return 0;
        };
        let elapsed_ns = now.duration_since(start).as_nanos() as u64;
        let owed = elapsed_ns / self.expected_frame_ns;
        let deficit = owed.saturating_sub(self.frame_index);
        // Report only the growth since last time. Recovering (deficit
        // shrinking) resets the baseline so a later shortfall is caught
        // again, but never retroactively un-counts a reported drop.
        let new = deficit.saturating_sub(self.reported_deficit);
        self.reported_deficit = deficit;
        new as u32
    }

    /// Frame durations in chronological order (oldest first).
    pub fn durations_recent_ns(&self) -> impl Iterator<Item = u64> + '_ {
        let n = self.valid_count.min(FRAME_HISTORY_SIZE);
        let start = (self.ring_head + FRAME_HISTORY_SIZE - n) % FRAME_HISTORY_SIZE;
        (0..n).map(move |i| self.durations_ns[(start + i) % FRAME_HISTORY_SIZE])
    }

    pub fn expected_ns(&self) -> u64 {
        self.expected_frame_ns
    }

    /// Reset the cumulative drop counter to zero (e.g. before a benchmark).
    pub fn reset_drops(&mut self) {
        self.drop_count = 0;
    }

    pub fn summary(&self) -> FrameSummary {
        let durations = &self.durations_ns[..self.valid_count.min(FRAME_HISTORY_SIZE)];
        if durations.is_empty() {
            return FrameSummary {
                fps: 0.0,
                mean_ms: 0.0,
                std_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                drop_count: self.drop_count,
                frame_index: self.frame_index,
            };
        }
        let n = durations.len() as f64;
        let mean_ns = durations.iter().sum::<u64>() as f64 / n;
        let var_ns = durations
            .iter()
            .map(|&d| {
                let x = d as f64 - mean_ns;
                x * x
            })
            .sum::<f64>()
            / n;
        FrameSummary {
            fps: if mean_ns > 0.0 {
                1_000_000_000.0 / mean_ns
            } else {
                0.0
            },
            mean_ms: mean_ns / 1_000_000.0,
            std_ms: var_ns.sqrt() / 1_000_000.0,
            min_ms: *durations.iter().min().unwrap() as f64 / 1_000_000.0,
            max_ms: *durations.iter().max().unwrap() as f64 / 1_000_000.0,
            drop_count: self.drop_count,
            frame_index: self.frame_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    const HZ: f64 = 60.0;
    const PERIOD: Duration = Duration::from_nanos(16_666_666);

    /// Feed `intervals` into `stats` and return the total drops reported.
    fn replay(stats: &mut FrameStats, intervals: &[Duration]) -> u32 {
        let mut t = Instant::now();
        stats.on_present(t);
        let mut total = 0;
        for d in intervals {
            t += *d;
            total += stats.on_present(t);
        }
        total
    }

    #[test]
    fn vblank_pacing_flags_a_missed_refresh() {
        let mut s = FrameStats::new(HZ);
        // One interval of two periods = one vblank missed.
        let drops = replay(&mut s, &[PERIOD, PERIOD * 2, PERIOD]);
        assert_eq!(drops, 1);
    }

    #[test]
    fn rate_averaged_ignores_jitter_that_keeps_up() {
        // Alternating 8/25 ms — the shape actually measured on evdi. Mean is
        // one period, so nothing has been lost and nothing should be reported,
        // even though half the intervals exceed the 1.25x vblank threshold.
        let short = Duration::from_micros(8_333);
        let long = Duration::from_micros(25_000);
        let intervals: Vec<Duration> = (0..600)
            .map(|i| if i % 2 == 0 { short } else { long })
            .collect();

        let mut averaged = FrameStats::new_rate_averaged(HZ);
        assert_eq!(
            replay(&mut averaged, &intervals),
            0,
            "bursty delivery at the target rate is not frame loss"
        );

        // The same input under vblank pacing is a storm of false positives —
        // this is the behaviour that made the evdi logs unreadable.
        let mut vblank = FrameStats::new(HZ);
        assert!(
            replay(&mut vblank, &intervals) > 100,
            "per-interval detection is what misreports bursty delivery"
        );
    }

    #[test]
    fn rate_averaged_still_reports_a_real_shortfall() {
        // A sustained half-rate link: 600 intervals of 2 periods is 300
        // frames' worth of loss over the same wall-clock.
        let intervals = vec![PERIOD * 2; 600];
        let mut s = FrameStats::new_rate_averaged(HZ);
        let drops = replay(&mut s, &intervals);
        assert!(
            (595..=600).contains(&drops),
            "half-rate delivery must be reported, got {drops}"
        );
    }

    #[test]
    fn rate_averaged_reports_a_stall_then_stops_once_recovered() {
        let mut s = FrameStats::new_rate_averaged(HZ);
        // Steady, then one 10-period stall, then steady again.
        let mut intervals = vec![PERIOD; 60];
        intervals.push(PERIOD * 10);
        let during = replay(&mut s, &intervals);
        assert!(during >= 8, "a real stall must be reported, got {during}");

        // Continuing at the target rate reports nothing further: the deficit
        // is already accounted for and must not be re-reported every frame.
        let mut t = Instant::now();
        let mut after = 0;
        for _ in 0..120 {
            t += PERIOD;
            after += s.on_present(t);
        }
        assert_eq!(after, 0, "an already-reported deficit must not repeat");
    }

    #[test]
    fn zero_or_invalid_refresh_never_panics() {
        for hz in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut s = FrameStats::new(hz);
            assert_eq!(replay(&mut s, &[PERIOD, PERIOD * 4]), 0);
            let mut s = FrameStats::new_rate_averaged(hz);
            assert_eq!(replay(&mut s, &[PERIOD, PERIOD * 4]), 0);
        }
    }
}
