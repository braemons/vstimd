"""Unit tests for analysis.py — no hardware required.

All tests use SimulatedBackend to generate synthetic photodiode data.
"""

import numpy as np
import pytest

from vstimd_timing_test.analysis import (
    AnalysisResult,
    Thresholds,
    analyze,
    detect_drops,
    detect_edges,
)
from vstimd_timing_test.backends.simulation import SimulatedBackend


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_data(
    hz: float = 60.0,
    duration_s: float = 5.0,
    drop_indices: list[int] | None = None,
    noise_std: float = 0.02,
) -> tuple[np.ndarray, np.ndarray, int]:
    """Return (timestamps_s, voltages_v, n_flashes_expected)."""
    backend = SimulatedBackend(
        hz=hz,
        duration_s=duration_s,
        drop_indices=drop_indices,
        noise_std=noise_std,
    )
    backend.start_acquisition()
    timestamps_s, voltages_v = backend.stop_acquisition()
    backend.close()
    n_expected = int(duration_s * hz)
    return timestamps_s, voltages_v, n_expected


# ---------------------------------------------------------------------------
# detect_edges
# ---------------------------------------------------------------------------

class TestDetectEdges:
    def test_perfect_signal_detects_all_edges(self):
        ts, vs, n = make_data(hz=60.0, duration_s=3.0)
        edges = detect_edges(ts, vs)
        # Allow ±1 edge for boundary effects
        assert abs(len(edges) - n) <= 2

    def test_no_signal_returns_empty(self):
        ts = np.linspace(0, 1, 1000)
        vs = np.zeros(1000)
        edges = detect_edges(ts, vs)
        assert len(edges) == 0

    def test_constant_high_returns_empty(self):
        ts = np.linspace(0, 1, 1000)
        vs = np.full(1000, 3.3)
        edges = detect_edges(ts, vs)
        assert len(edges) == 0

    def test_single_pulse(self):
        sample_rate = 10_000
        ts = np.linspace(0, 0.1, sample_rate)
        vs = np.zeros(sample_rate)
        # Pulse from 0.02 s to 0.05 s
        vs[(ts >= 0.02) & (ts < 0.05)] = 3.3
        edges = detect_edges(ts, vs)
        assert len(edges) == 1
        assert abs(edges[0] - 0.02) < 0.001  # within 1 ms

    def test_sub_sample_accuracy(self):
        """Edge time should be close to the true edge time."""
        sample_rate = 1_000
        ts = np.linspace(0, 1, sample_rate)
        vs = np.zeros(sample_rate, dtype=np.float64)
        true_edge = 0.3337  # not on a sample boundary
        vs[ts >= true_edge] = 3.3
        edges = detect_edges(ts, vs, threshold_v=1.5)
        assert len(edges) == 1
        assert abs(edges[0] - true_edge) < 1.5 / sample_rate  # sub-sample accuracy

    def test_noisy_signal_detects_edges(self):
        ts, vs, n = make_data(hz=30.0, duration_s=2.0, noise_std=0.1)
        edges = detect_edges(ts, vs, threshold_v=1.5)
        assert abs(len(edges) - n) <= 2

    def test_debounce_removes_spurious_edges(self):
        """Multiple crossings within min_separation should produce only one edge."""
        sample_rate = 10_000
        ts = np.linspace(0, 0.1, sample_rate)
        # Create a noisy rising edge that crosses threshold 3 times within 2 ms
        vs = np.zeros(sample_rate, dtype=np.float64)
        # Rapid oscillation near threshold for 1 ms
        idx_start = int(0.02 * sample_rate)
        idx_end = int(0.021 * sample_rate)
        vs[idx_start:idx_end] = np.array([0.5, 2.0, 0.8, 2.5, 1.0, 3.3])[:idx_end - idx_start]
        vs[idx_end:] = 3.3
        edges = detect_edges(ts, vs, threshold_v=1.5, min_separation_s=0.005)
        assert len(edges) == 1


# ---------------------------------------------------------------------------
# detect_drops
# ---------------------------------------------------------------------------

class TestDetectDrops:
    def test_no_drops(self):
        edges = np.arange(0, 10) * (1.0 / 60.0)
        drops, indices = detect_drops(edges, expected_ipi_s=1.0 / 60.0)
        assert drops == 0
        assert indices == []

    def test_single_drop(self):
        ipi = 1.0 / 60.0
        edges = np.array([i * ipi for i in range(10)])
        # Remove edge at index 5 → gap of 2×IPI between edges 4 and 6
        edges = np.concatenate([edges[:5], edges[6:]])
        drops, indices = detect_drops(edges, expected_ipi_s=ipi)
        assert drops == 1
        assert 5 in indices

    def test_two_consecutive_drops(self):
        ipi = 1.0 / 60.0
        edges = np.array([i * ipi for i in range(10)])
        # Remove edges 4 and 5 → gap of 3×IPI
        edges = np.concatenate([edges[:4], edges[6:]])
        drops, indices = detect_drops(edges, expected_ipi_s=ipi)
        assert drops == 2

    def test_empty_input(self):
        drops, indices = detect_drops(np.array([]), expected_ipi_s=1.0 / 60.0)
        assert drops == 0
        assert indices == []

    def test_single_edge(self):
        drops, indices = detect_drops(np.array([0.0]), expected_ipi_s=1.0 / 60.0)
        assert drops == 0
        assert indices == []


# ---------------------------------------------------------------------------
# Full pipeline via analyze()
# ---------------------------------------------------------------------------

class TestAnalyze:
    def test_300_flashes_no_drops_pass(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.dropped_count == 0
        assert result.ipi_std_ms < 1.0  # generous for simulation noise
        assert result.verdict == "PASS"

    def test_inject_2_drops_counted(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0, drop_indices=[10, 50])
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.dropped_count == 2
        assert result.verdict in ("WARN", "FAIL")  # 2 drops → WARN per default thresholds

    def test_inject_3_drops_fail(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0, drop_indices=[5, 25, 100])
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.dropped_count == 3
        assert result.verdict == "FAIL"

    def test_ipi_approximately_correct(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        # IPI should be close to 1000/60 ≈ 16.67 ms
        expected_ipi_ms = 1000.0 / 60.0
        assert abs(result.ipi_mean_ms - expected_ipi_ms) < 1.0

    def test_frame_rate_near_nominal(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert abs(result.frame_rate_measured_hz - 60.0) < 0.5

    def test_custom_thresholds_fail_on_1_drop(self):
        ts, vs, n = make_data(hz=60.0, duration_s=5.0, drop_indices=[10])
        strict = Thresholds(drop_warn=0, drop_fail=1)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0, thresholds=strict)
        assert result.verdict == "FAIL"

    def test_30hz(self):
        ts, vs, n = make_data(hz=30.0, duration_s=5.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=30.0)
        assert result.dropped_count == 0
        assert result.verdict == "PASS"
        expected_ipi_ms = 1000.0 / 30.0
        assert abs(result.ipi_mean_ms - expected_ipi_ms) < 1.0

    def test_result_fields_populated(self):
        ts, vs, n = make_data(hz=60.0, duration_s=3.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.n_flashes_expected == n
        assert result.n_edges_detected > 0
        assert result.expected_ipi_ms == pytest.approx(1000.0 / 60.0)
        assert result.latency_source == "none"  # no flip timestamps given
