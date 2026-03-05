"""Colored terminal report for analysis results."""

from __future__ import annotations

from .analysis import AnalysisResult

try:
    from colorama import Fore, Style, init as colorama_init  # type: ignore[import]
    colorama_init(autoreset=True)
    _HAS_COLORAMA = True
except ImportError:
    _HAS_COLORAMA = False


def _color(text: str, color: str) -> str:
    if not _HAS_COLORAMA:
        return text
    return f"{color}{text}{Style.RESET_ALL}"


def _verdict_color(verdict: str) -> str:
    if not _HAS_COLORAMA:
        return verdict
    mapping = {
        "PASS": Fore.GREEN,
        "WARN": Fore.YELLOW,
        "FAIL": Fore.RED,
    }
    return _color(f"[ {verdict} ]", mapping.get(verdict, ""))


def _metric_color(value: float, warn: float, fail: float, fmt: str = ".2f") -> str:
    formatted = f"{value:{fmt}}"
    if not _HAS_COLORAMA:
        return formatted
    if value >= fail:
        return _color(formatted, Fore.RED)
    if value >= warn:
        return _color(formatted, Fore.YELLOW)
    return _color(formatted, Fore.GREEN)


def print_report(result: AnalysisResult) -> None:
    """Print a human-readable timing report to stdout."""
    print()
    print("=" * 60)
    print(f"  Frame Timing Verification Report")
    print("=" * 60)

    print(f"\n  Verdict: {_verdict_color(result.verdict)}")

    if result.failure_reasons:
        for reason in result.failure_reasons:
            print(f"    {'FAIL' if result.verdict == 'FAIL' else 'WARN'}: {reason}")

    print(f"\n  {'Metric':<35} {'Value':>12}  {'Expected':>12}")
    print(f"  {'-'*35} {'-'*12}  {'-'*12}")

    drops_color = _metric_color(result.dropped_count, 1, 3, ".0f")
    print(f"  {'Dropped frames':<35} {drops_color:>12}  {'0':>12}")

    jitter_color = _metric_color(result.ipi_std_ms, 0.3, 1.0)
    print(f"  {'Jitter (IPI std dev, ms)':<35} {jitter_color:>12}  {'< 0.3 ms':>12}")

    ipi_color = _metric_color(result.ipi_mean_ms, 0, 0)
    print(f"  {'Mean IPI (ms)':<35} {ipi_color:>12}  {result.expected_ipi_ms:>12.2f}")

    print(f"  {'IPI min/max (ms)':<35} {result.ipi_min_ms:>12.2f}  {result.ipi_max_ms:>12.2f}")

    if result.render_to_photon_latency_ms > 0:
        lat_color = _metric_color(result.render_to_photon_latency_ms, 10.0, 20.0)
        print(f"  {'Render→photon latency (ms)':<35} {lat_color:>12}  {'< 10 ms':>12}")
        print(f"  {'Latency std dev (ms)':<35} {result.latency_std_ms:>12.2f}")

    if result.zmq_cmd_to_photon_latency_ms > 0:
        print(f"  {'ZMQ cmd→photon latency (ms)':<35} {result.zmq_cmd_to_photon_latency_ms:>12.2f}")

    hz_color = _metric_color(
        abs(result.frame_rate_measured_hz - (1000.0 / result.expected_ipi_ms if result.expected_ipi_ms > 0 else 0)),
        0.1, 0.5
    )
    print(f"  {'Measured frame rate (Hz)':<35} {result.frame_rate_measured_hz:>12.2f}")
    print(f"  {'Edges detected / expected':<35} {result.n_edges_detected:>12}  {result.n_flashes_expected:>12}")

    if result.clock_offset_ms != 0.0:
        print(f"  {'Clock offset Python↔server (ms)':<35} {result.clock_offset_ms:>12.2f}")

    print(f"  {'Latency source':<35} {result.latency_source:>12}")

    if result.dropped_indices:
        print(f"\n  Drops at flash indices: {result.dropped_indices}")

    print()
    print("=" * 60)
    print()


def classify_verdict(result: AnalysisResult) -> str:
    """Return 'PASS', 'WARN', or 'FAIL'."""
    return result.verdict
