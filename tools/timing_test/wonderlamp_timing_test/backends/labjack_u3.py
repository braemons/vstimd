"""LabJack U3 backend (poll-based, ~1 kHz effective rate).

Install: uv pip install -e tools/timing_test[u3]

The U3 does not support hardware-buffered analog acquisition, so this backend
uses a busy-wait loop to achieve < 100 µs inter-sample jitter on Windows.
"""

from __future__ import annotations

import time
from typing import TYPE_CHECKING


class LabJackU3Backend:
    """Acquire photodiode data from a LabJack U3 via polled reads.

    Parameters
    ----------
    channel:
        Analog input channel index (0–7 for AIN0–AIN7).
    sample_rate_hz:
        Target sample rate.  Max reliable rate is ~1 000 Hz on Windows.
    """

    def __init__(
        self,
        channel: int = 0,
        sample_rate_hz: int = 1_000,
    ) -> None:
        try:
            import u3  # type: ignore[import]
        except ImportError as e:
            raise ImportError(
                "labjack package not installed — run: uv pip install -e tools/timing_test[u3]"
            ) from e

        self._channel = channel
        self._sample_rate = sample_rate_hz
        self._u3_mod = u3
        self._device = None
        self._running = False
        self._samples: list[tuple[float, float]] = []  # (timestamp_s, voltage_v)

    # --- Protocol -----------------------------------------------------------

    @property
    def name(self) -> str:
        return f"LabJackU3Backend(AIN{self._channel})"

    @property
    def sample_rate_hz(self) -> int:
        return self._sample_rate

    def start_acquisition(self) -> None:
        self._device = self._u3_mod.U3()
        self._device.getCalibrationData()
        self._running = True
        self._samples = []

        interval_s = 1.0 / self._sample_rate
        next_sample = time.perf_counter()

        while self._running:
            # Busy-wait for precise timing
            while time.perf_counter() < next_sample:
                pass
            t = time.perf_counter()
            raw = self._device.getAIN(self._channel)
            self._samples.append((t, raw))
            next_sample += interval_s

    def stop_acquisition(self):
        import numpy as np

        self._running = False
        if self._device is not None:
            self._device.close()
            self._device = None

        if not self._samples:
            return np.array([], dtype=np.float64), np.array([], dtype=np.float64)

        t0 = self._samples[0][0]
        timestamps = np.array([s[0] - t0 for s in self._samples], dtype=np.float64)
        voltages = np.array([s[1] for s in self._samples], dtype=np.float64)
        return timestamps, voltages

    def get_device_info(self) -> dict:
        return {
            "name": self.name,
            "channel": self._channel,
            "sample_rate_hz": self._sample_rate,
        }

    def close(self) -> None:
        self._running = False
        if self._device is not None:
            try:
                self._device.close()
            except Exception:
                pass
            self._device = None
