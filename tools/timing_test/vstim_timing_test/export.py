"""Export analysis results to CSV and JSON."""

from __future__ import annotations

import csv
import json
import time
from pathlib import Path

from .analysis import AnalysisResult


def export_csv(result: AnalysisResult, path: str | Path) -> Path:
    """Write per-metric rows to a CSV file.

    Each row: metric_name, value, unit
    Returns the resolved Path.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)

    rows = [
        ("verdict", result.verdict, ""),
        ("n_flashes_expected", result.n_flashes_expected, "count"),
        ("n_edges_detected", result.n_edges_detected, "count"),
        ("dropped_count", result.dropped_count, "count"),
        ("ipi_mean_ms", result.ipi_mean_ms, "ms"),
        ("ipi_std_ms", result.ipi_std_ms, "ms"),
        ("ipi_min_ms", result.ipi_min_ms, "ms"),
        ("ipi_max_ms", result.ipi_max_ms, "ms"),
        ("expected_ipi_ms", result.expected_ipi_ms, "ms"),
        ("render_to_photon_latency_ms", result.render_to_photon_latency_ms, "ms"),
        ("latency_std_ms", result.latency_std_ms, "ms"),
        ("zmq_cmd_to_photon_latency_ms", result.zmq_cmd_to_photon_latency_ms, "ms"),
        ("frame_rate_measured_hz", result.frame_rate_measured_hz, "Hz"),
        ("clock_offset_ms", result.clock_offset_ms, "ms"),
        ("latency_source", result.latency_source, ""),
    ]

    with open(path, "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["metric", "value", "unit"])
        writer.writerows(rows)

    return path


def export_json(
    result: AnalysisResult,
    path: str | Path,
    metadata: dict | None = None,
) -> Path:
    """Write full result + metadata to a JSON file.

    Parameters
    ----------
    result:
        Analysis result to export.
    path:
        Output file path.
    metadata:
        Optional extra metadata (device info, git hash, etc.) merged into the
        top-level JSON object.
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)

    data: dict = {
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "verdict": result.verdict,
        "failure_reasons": result.failure_reasons,
        "metrics": {
            "n_flashes_expected": result.n_flashes_expected,
            "n_edges_detected": result.n_edges_detected,
            "dropped_count": result.dropped_count,
            "dropped_indices": result.dropped_indices,
            "ipi_mean_ms": result.ipi_mean_ms,
            "ipi_std_ms": result.ipi_std_ms,
            "ipi_min_ms": result.ipi_min_ms,
            "ipi_max_ms": result.ipi_max_ms,
            "expected_ipi_ms": result.expected_ipi_ms,
            "render_to_photon_latency_ms": result.render_to_photon_latency_ms,
            "latency_std_ms": result.latency_std_ms,
            "zmq_cmd_to_photon_latency_ms": result.zmq_cmd_to_photon_latency_ms,
            "frame_rate_measured_hz": result.frame_rate_measured_hz,
            "clock_offset_ms": result.clock_offset_ms,
            "latency_source": result.latency_source,
        },
    }

    if metadata:
        data["metadata"] = metadata

    with open(path, "w") as f:
        json.dump(data, f, indent=2)

    return path
