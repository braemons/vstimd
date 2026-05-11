"""Signal analysis for frame timing verification.

Rising edge detection uses a Schmitt-trigger style approach with sub-sample
interpolation.  Dropped frame detection compares inter-pulse intervals
against the expected interval.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import numpy as np


# ---------------------------------------------------------------------------
# Result dataclass
# ---------------------------------------------------------------------------

@dataclass
class AnalysisResult:
    n_flashes_expected: int
    n_edges_detected: int
    dropped_count: int
    dropped_indices: list[int]
    ipi_mean_ms: float
    ipi_std_ms: float
    ipi_min_ms: float
    ipi_max_ms: float
    expected_ipi_ms: float
    render_to_photon_latency_ms: float
    latency_std_ms: float
    zmq_cmd_to_photon_latency_ms: float
    frame_rate_measured_hz: float
    clock_offset_ms: float
    latency_source: str
    verdict: str
    failure_reasons: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Thresholds (can be overridden by caller)
# ---------------------------------------------------------------------------

@dataclass
class Thresholds:
    drop_warn: int = 1          # drops → WARN
    drop_fail: int = 3          # drops → FAIL
    jitter_warn_ms: float = 0.3
    jitter_fail_ms: float = 1.0
    ipi_ratio_warn: float = 1.2
    ipi_ratio_fail: float = 1.8
    latency_warn_ms: float = 10.0
    latency_fail_ms: float = 20.0
    hz_drift_warn: float = 0.1
    hz_drift_fail: float = 0.5


# ---------------------------------------------------------------------------
# Edge detection
# ---------------------------------------------------------------------------

def detect_edges(
    timestamps_s: np.ndarray,
    voltages_v: np.ndarray,
    threshold_v: float = 1.5,
    min_separation_s: float = 0.005,
    smooth_window: int = 3,
) -> np.ndarray:
    """Return sub-sample-accurate rising-edge timestamps (seconds).

    Algorithm:
    1. 3-point moving-average smoothing to reduce noise.
    2. Schmitt-trigger: detect below→above threshold crossings.
    3. Linear interpolation for sub-sample accuracy.
    4. Enforce minimum separation (debounce).
    """
    if len(timestamps_s) < smooth_window:
        return np.array([], dtype=np.float64)

    # 1. Smooth
    kernel = np.ones(smooth_window) / smooth_window
    v = np.convolve(voltages_v.astype(np.float64), kernel, mode="same")

    # 2. Find crossings: sample i-1 below threshold, sample i above threshold
    below = v[:-1] < threshold_v
    above = v[1:] >= threshold_v
    crossing_indices = np.where(below & above)[0]  # index of the sample just before crossing

    if len(crossing_indices) == 0:
        return np.array([], dtype=np.float64)

    # 3. Sub-sample interpolation
    edge_times: list[float] = []
    for idx in crossing_indices:
        v0 = v[idx]
        v1 = v[idx + 1]
        if v1 == v0:
            t_edge = float(timestamps_s[idx])
        else:
            frac = (threshold_v - v0) / (v1 - v0)
            dt = float(timestamps_s[idx + 1]) - float(timestamps_s[idx])
            t_edge = float(timestamps_s[idx]) + frac * dt
        edge_times.append(t_edge)

    if not edge_times:
        return np.array([], dtype=np.float64)

    # 4. Debounce
    result: list[float] = [edge_times[0]]
    for t in edge_times[1:]:
        if t - result[-1] >= min_separation_s:
            result.append(t)

    return np.array(result, dtype=np.float64)


# ---------------------------------------------------------------------------
# Drop detection
# ---------------------------------------------------------------------------

def detect_drops(
    edge_times_s: np.ndarray,
    expected_ipi_s: float,
    drop_threshold_ratio: float = 1.5,
) -> tuple[int, list[int]]:
    """Count dropped frames and return (total_drops, list_of_flash_indices).

    A gap of N × expected_ipi between consecutive edges means N-1 drops.
    Returns (total_drop_count, indices_of_flashes_after_which_drops_occurred).
    """
    if len(edge_times_s) < 2:
        return 0, []

    ipis = np.diff(edge_times_s)
    total_drops = 0
    drop_indices: list[int] = []

    for i, ipi in enumerate(ipis):
        ratio = ipi / expected_ipi_s
        if ratio > drop_threshold_ratio:
            n_drops = round(ratio) - 1
            if n_drops < 1:
                n_drops = 1
            total_drops += n_drops
            drop_indices.append(i + 1)  # flash index after which drops occurred

    return total_drops, drop_indices


# ---------------------------------------------------------------------------
# Main analysis entry point
# ---------------------------------------------------------------------------

def analyze(
    timestamps_s: np.ndarray,
    voltages_v: np.ndarray,
    n_flashes_expected: int,
    expected_hz: float,
    cmd_timestamps_s: np.ndarray | None = None,
    flip_timestamps_s: np.ndarray | None = None,
    clock_offset_ms: float = 0.0,
    thresholds: Thresholds | None = None,
    threshold_v: float = 1.5,
) -> AnalysisResult:
    """Full analysis pipeline.

    Parameters
    ----------
    timestamps_s:
        DAQ sample timestamps in seconds.
    voltages_v:
        DAQ photodiode voltage samples.
    n_flashes_expected:
        How many flashes the protocol sent.
    expected_hz:
        Target flash rate (same as stimulus Hz).
    cmd_timestamps_s:
        When each ZMQ command was sent (optional; for zmq_cmd_to_photon).
    flip_timestamps_s:
        Server flip timestamps in seconds, after clock_offset correction (optional).
    clock_offset_ms:
        Python↔server clock offset measured by calibration (ms).
    thresholds:
        Pass/warn/fail thresholds. Uses defaults if None.
    threshold_v:
        Schmitt-trigger threshold voltage.
    """
    if thresholds is None:
        thresholds = Thresholds()

    expected_ipi_s = 1.0 / expected_hz
    expected_ipi_ms = expected_ipi_s * 1000.0

    edges = detect_edges(timestamps_s, voltages_v, threshold_v=threshold_v)
    n_edges = len(edges)

    dropped_count, dropped_indices = detect_drops(edges, expected_ipi_s)

    # IPI stats
    if n_edges >= 2:
        ipis_ms = np.diff(edges) * 1000.0
        ipi_mean_ms = float(np.mean(ipis_ms))
        ipi_std_ms = float(np.std(ipis_ms))
        ipi_min_ms = float(np.min(ipis_ms))
        ipi_max_ms = float(np.max(ipis_ms))
    else:
        ipi_mean_ms = ipi_std_ms = ipi_min_ms = ipi_max_ms = 0.0

    # Frame rate from edges
    if n_edges >= 2:
        total_time_s = edges[-1] - edges[0]
        frame_rate_measured_hz = (n_edges - 1) / total_time_s if total_time_s > 0 else 0.0
    else:
        frame_rate_measured_hz = 0.0

    # Render-to-photon latency (flip timestamps → photodiode edge)
    render_to_photon_latency_ms = 0.0
    latency_std_ms = 0.0
    latency_source = "none"

    if flip_timestamps_s is not None and n_edges >= 1:
        # Match each edge to the nearest preceding flip timestamp
        n_pairs = min(n_edges, len(flip_timestamps_s))
        latencies_ms = []
        for i in range(n_pairs):
            latency = (edges[i] - flip_timestamps_s[i]) * 1000.0 + clock_offset_ms
            if 0 <= latency < 200:  # sanity check: 0–200 ms
                latencies_ms.append(latency)
        if latencies_ms:
            render_to_photon_latency_ms = float(np.mean(latencies_ms))
            latency_std_ms = float(np.std(latencies_ms))
            latency_source = "software_zmq"

    # ZMQ cmd-to-photon latency (reference metric)
    zmq_cmd_to_photon_latency_ms = 0.0
    if cmd_timestamps_s is not None and n_edges >= 1:
        n_pairs = min(n_edges, len(cmd_timestamps_s))
        cmd_latencies_ms = [
            (edges[i] - cmd_timestamps_s[i]) * 1000.0
            for i in range(n_pairs)
            if 0 <= (edges[i] - cmd_timestamps_s[i]) * 1000.0 < 500
        ]
        if cmd_latencies_ms:
            zmq_cmd_to_photon_latency_ms = float(np.mean(cmd_latencies_ms))

    # Verdict
    failure_reasons: list[str] = []
    warn_reasons: list[str] = []

    if dropped_count >= thresholds.drop_fail:
        failure_reasons.append(f"dropped_count={dropped_count} ≥ {thresholds.drop_fail}")
    elif dropped_count >= thresholds.drop_warn:
        warn_reasons.append(f"dropped_count={dropped_count}")

    if ipi_std_ms > thresholds.jitter_fail_ms:
        failure_reasons.append(f"ipi_std_ms={ipi_std_ms:.3f} > {thresholds.jitter_fail_ms}")
    elif ipi_std_ms > thresholds.jitter_warn_ms:
        warn_reasons.append(f"ipi_std_ms={ipi_std_ms:.3f}")

    if expected_ipi_ms > 0:
        ipi_ratio = ipi_max_ms / expected_ipi_ms if expected_ipi_ms > 0 else 0.0
        if ipi_ratio > thresholds.ipi_ratio_fail:
            failure_reasons.append(f"ipi_max/expected={ipi_ratio:.2f}× > {thresholds.ipi_ratio_fail}×")
        elif ipi_ratio > thresholds.ipi_ratio_warn:
            warn_reasons.append(f"ipi_max/expected={ipi_ratio:.2f}×")

    if render_to_photon_latency_ms > thresholds.latency_fail_ms:
        failure_reasons.append(
            f"render_to_photon={render_to_photon_latency_ms:.1f} ms > {thresholds.latency_fail_ms} ms"
        )
    elif render_to_photon_latency_ms > thresholds.latency_warn_ms and latency_source != "none":
        warn_reasons.append(f"render_to_photon={render_to_photon_latency_ms:.1f} ms")

    if expected_hz > 0:
        hz_drift = abs(frame_rate_measured_hz - expected_hz)
        if hz_drift > thresholds.hz_drift_fail:
            failure_reasons.append(f"hz_drift={hz_drift:.3f} > {thresholds.hz_drift_fail}")
        elif hz_drift > thresholds.hz_drift_warn:
            warn_reasons.append(f"hz_drift={hz_drift:.3f}")

    if failure_reasons:
        verdict = "FAIL"
        all_reasons = failure_reasons
    elif warn_reasons:
        verdict = "WARN"
        all_reasons = warn_reasons
    else:
        verdict = "PASS"
        all_reasons = []

    return AnalysisResult(
        n_flashes_expected=n_flashes_expected,
        n_edges_detected=n_edges,
        dropped_count=dropped_count,
        dropped_indices=dropped_indices,
        ipi_mean_ms=ipi_mean_ms,
        ipi_std_ms=ipi_std_ms,
        ipi_min_ms=ipi_min_ms,
        ipi_max_ms=ipi_max_ms,
        expected_ipi_ms=expected_ipi_ms,
        render_to_photon_latency_ms=render_to_photon_latency_ms,
        latency_std_ms=latency_std_ms,
        zmq_cmd_to_photon_latency_ms=zmq_cmd_to_photon_latency_ms,
        frame_rate_measured_hz=frame_rate_measured_hz,
        clock_offset_ms=clock_offset_ms,
        latency_source=latency_source,
        verdict=verdict,
        failure_reasons=all_reasons,
    )
