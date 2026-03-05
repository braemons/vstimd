"""Simulated DAQ backend — no hardware required.

Generates synthetic photodiode voltage data that mimics a real photodiode
recording flashes from the screen.  Useful for CI / unit tests.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

import numpy as np


class SimulatedBackend:
    """Generate synthetic square-wave photodiode data.

    Parameters
    ----------
    hz:
        Flash rate in Hz.
    duration_s:
        How long to "record" (seconds).
    sample_rate_hz:
        DAQ sample rate (default 10 000 Hz).
    noise_std:
        Gaussian noise standard deviation (V).
    drop_indices:
        Flash indices at which to suppress the rising edge (simulate drops).
    flash_duty:
        Fraction of the period the flash is "on" (0–1).
    v_low / v_high:
        Voltage levels for off / on.
    """

    def __init__(
        self,
        hz: float = 60.0,
        duration_s: float = 5.0,
        sample_rate_hz: int = 10_000,
        noise_std: float = 0.05,
        drop_indices: list[int] | None = None,
        flash_duty: float = 0.5,
        v_low: float = 0.0,
        v_high: float = 3.3,
    ) -> None:
        self._hz = hz
        self._duration_s = duration_s
        self._sample_rate = sample_rate_hz
        self._noise_std = noise_std
        self._drop_indices: set[int] = set(drop_indices or [])
        self._flash_duty = flash_duty
        self._v_low = v_low
        self._v_high = v_high
        self._started_at: float | None = None

    # --- Protocol -----------------------------------------------------------

    @property
    def name(self) -> str:
        return "SimulatedBackend"

    @property
    def sample_rate_hz(self) -> int:
        return self._sample_rate

    def start_acquisition(self) -> None:
        self._started_at = time.perf_counter()

    def stop_acquisition(self) -> tuple[np.ndarray, np.ndarray]:
        """Return (timestamps_s, voltages_v)."""
        n_samples = int(self._duration_s * self._sample_rate)
        timestamps_s = np.linspace(0.0, self._duration_s, n_samples, endpoint=False)

        period_s = 1.0 / self._hz
        # Each flash starts at flash_idx * period_s
        n_flashes = int(self._duration_s * self._hz)
        on_duration_s = period_s * self._flash_duty

        voltages = np.full(n_samples, self._v_low, dtype=np.float64)

        for flash_idx in range(n_flashes):
            if flash_idx in self._drop_indices:
                continue
            t_on = flash_idx * period_s
            t_off = t_on + on_duration_s
            mask = (timestamps_s >= t_on) & (timestamps_s < t_off)
            voltages[mask] = self._v_high

        # Add Gaussian noise
        rng = np.random.default_rng(seed=42)
        voltages += rng.normal(0.0, self._noise_std, n_samples)

        return timestamps_s, voltages

    def get_device_info(self) -> dict:
        return {
            "name": self.name,
            "sample_rate_hz": self._sample_rate,
            "hz": self._hz,
            "duration_s": self._duration_s,
            "drop_indices": sorted(self._drop_indices),
        }

    def close(self) -> None:
        pass
