"""Unit tests for threshold classification in report.py."""

from __future__ import annotations

from vstimd_timing_test.analysis import AnalysisResult, Thresholds, analyze
from vstimd_timing_test.backends.simulation import SimulatedBackend
from vstimd_timing_test.report import classify_verdict


def _make_result(**kwargs) -> AnalysisResult:
    """Build a minimal AnalysisResult with sane defaults, overriding via kwargs."""
    defaults = dict(
        n_flashes_expected=300,
        n_edges_detected=300,
        dropped_count=0,
        dropped_indices=[],
        ipi_mean_ms=16.67,
        ipi_std_ms=0.1,
        ipi_min_ms=16.0,
        ipi_max_ms=17.0,
        expected_ipi_ms=16.67,
        render_to_photon_latency_ms=0.0,
        latency_std_ms=0.0,
        zmq_cmd_to_photon_latency_ms=0.0,
        frame_rate_measured_hz=60.0,
        clock_offset_ms=0.0,
        latency_source="none",
        verdict="PASS",
        failure_reasons=[],
    )
    defaults.update(kwargs)
    return AnalysisResult(**defaults)


class TestVerdict:
    def test_clean_pass(self):
        result = _make_result()
        assert result.verdict == "PASS"
        assert classify_verdict(result) == "PASS"

    def test_1_drop_is_warn(self):
        backend = SimulatedBackend(hz=60.0, duration_s=5.0, drop_indices=[10])
        backend.start_acquisition()
        ts, vs = backend.stop_acquisition()
        backend.close()
        n = int(5.0 * 60.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.dropped_count == 1
        assert result.verdict == "WARN"

    def test_3_drops_is_fail(self):
        backend = SimulatedBackend(hz=60.0, duration_s=5.0, drop_indices=[5, 50, 100])
        backend.start_acquisition()
        ts, vs = backend.stop_acquisition()
        backend.close()
        n = int(5.0 * 60.0)
        result = analyze(ts, vs, n_flashes_expected=n, expected_hz=60.0)
        assert result.dropped_count >= 3
        assert result.verdict == "FAIL"

    def test_high_jitter_fail(self):
        result = _make_result(ipi_std_ms=2.0, verdict="FAIL", failure_reasons=["ipi_std_ms=2.000 > 1.0"])
        assert classify_verdict(result) == "FAIL"

    def test_medium_jitter_warn(self):
        result = _make_result(ipi_std_ms=0.5, verdict="WARN", failure_reasons=["ipi_std_ms=0.500"])
        assert classify_verdict(result) == "WARN"

    def test_high_latency_fail(self):
        result = _make_result(
            render_to_photon_latency_ms=25.0,
            latency_source="software_zmq",
            verdict="FAIL",
            failure_reasons=["render_to_photon=25.0 ms > 20.0 ms"],
        )
        assert classify_verdict(result) == "FAIL"

    def test_medium_latency_warn(self):
        result = _make_result(
            render_to_photon_latency_ms=15.0,
            latency_source="software_zmq",
            verdict="WARN",
            failure_reasons=["render_to_photon=15.0 ms"],
        )
        assert classify_verdict(result) == "WARN"

    def test_fail_beats_warn(self):
        """If any metric fails, verdict must be FAIL even if others only warn."""
        result = _make_result(
            ipi_std_ms=0.5,   # WARN level
            dropped_count=5,  # FAIL level
            verdict="FAIL",
            failure_reasons=["dropped_count=5 ≥ 3"],
        )
        assert classify_verdict(result) == "FAIL"
