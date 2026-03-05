"""LabJack T4/T7 backend (stream-based, up to 100 kHz).

Install: uv pip install -e tools/timing_test[t4]
"""

from __future__ import annotations

import time


class LabJackT4Backend:
    """Acquire photodiode data from a LabJack T4/T7 via LJM stream.

    The T4/T7 use hardware-buffered analog streaming with precise internal
    clock — much lower jitter than the U3 poll-based approach.

    Parameters
    ----------
    channel:
        Channel name string, e.g. ``"AIN0"``.
    sample_rate_hz:
        Stream sample rate (default 10 000 Hz; max 100 000 Hz for T7).
    """

    def __init__(
        self,
        channel: str = "AIN0",
        sample_rate_hz: int = 10_000,
    ) -> None:
        try:
            from labjack import ljm  # type: ignore[import]
        except ImportError as e:
            raise ImportError(
                "labjack-ljm not installed — run: uv pip install -e tools/timing_test[t4]"
            ) from e

        self._ljm = ljm
        self._channel = channel
        self._sample_rate = sample_rate_hz
        self._handle = None
        self._stream_started = False
        self._start_time: float | None = None
        self._voltages: list[float] = []

    # --- Protocol -----------------------------------------------------------

    @property
    def name(self) -> str:
        return f"LabJackT4Backend({self._channel})"

    @property
    def sample_rate_hz(self) -> int:
        return self._sample_rate

    def start_acquisition(self) -> None:
        ljm = self._ljm
        self._handle = ljm.openS("ANY", "ANY", "ANY")

        a_scan_list = ljm.namesToAddresses(1, [self._channel])[0]
        scan_rate = ljm.eStreamStart(
            self._handle,
            scans_per_read=self._sample_rate // 10,  # read ~100 ms chunks
            num_addresses=1,
            a_scan_list=[a_scan_list],
            scan_rate=self._sample_rate,
        )
        self._stream_started = True
        self._start_time = time.perf_counter()
        self._voltages = []
        self._actual_rate = scan_rate

    def stop_acquisition(self):
        import numpy as np

        if self._handle is None or not self._stream_started:
            raise RuntimeError("start_acquisition() was not called")

        # Drain remaining samples
        try:
            ret = self._ljm.eStreamRead(self._handle)
            self._voltages.extend(ret[0])
        except Exception:
            pass

        self._ljm.eStreamStop(self._handle)
        self._ljm.close(self._handle)
        self._handle = None
        self._stream_started = False

        voltages = np.array(self._voltages, dtype=np.float64)
        dt = 1.0 / self._sample_rate
        timestamps = np.arange(len(voltages), dtype=np.float64) * dt
        return timestamps, voltages

    def read_chunk(self) -> None:
        """Read a chunk from the stream buffer; call periodically during acquisition."""
        if self._handle is not None and self._stream_started:
            try:
                ret = self._ljm.eStreamRead(self._handle)
                self._voltages.extend(ret[0])
            except Exception:
                pass

    def get_device_info(self) -> dict:
        return {
            "name": self.name,
            "channel": self._channel,
            "sample_rate_hz": self._sample_rate,
        }

    def close(self) -> None:
        if self._handle is not None:
            try:
                if self._stream_started:
                    self._ljm.eStreamStop(self._handle)
                self._ljm.close(self._handle)
            except Exception:
                pass
            self._handle = None
