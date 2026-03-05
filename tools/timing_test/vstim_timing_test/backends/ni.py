"""National Instruments DAQmx backend (requires nidaqmx package).

Install: uv pip install -e tools/timing_test[ni]
"""

from __future__ import annotations


class NIBackend:
    """Read analog input from an NI DAQ device via nidaqmx.

    Parameters
    ----------
    device:
        Device name, e.g. ``"Dev1"``.
    channel:
        Physical channel, e.g. ``"ai0"``.
    sample_rate_hz:
        Acquisition sample rate (default 10 000 Hz).
    v_min / v_max:
        Input voltage range.
    """

    def __init__(
        self,
        device: str = "Dev1",
        channel: str = "ai0",
        sample_rate_hz: int = 10_000,
        v_min: float = -10.0,
        v_max: float = 10.0,
    ) -> None:
        try:
            import nidaqmx  # type: ignore[import]
            import nidaqmx.constants  # type: ignore[import]
        except ImportError as e:
            raise ImportError("nidaqmx not installed — run: uv pip install -e tools/timing_test[ni]") from e

        self._nidaqmx = nidaqmx
        self._device = device
        self._channel = channel
        self._sample_rate = sample_rate_hz
        self._v_min = v_min
        self._v_max = v_max
        self._task = None
        self._samples: list[float] = []

    # --- Protocol -----------------------------------------------------------

    @property
    def name(self) -> str:
        return f"NIBackend({self._device}/{self._channel})"

    @property
    def sample_rate_hz(self) -> int:
        return self._sample_rate

    def start_acquisition(self) -> None:
        import nidaqmx  # type: ignore[import]
        import nidaqmx.constants  # type: ignore[import]

        task = nidaqmx.Task()
        task.ai_channels.add_ai_voltage_chan(
            f"{self._device}/{self._channel}",
            min_val=self._v_min,
            max_val=self._v_max,
        )
        task.timing.cfg_samp_clk_timing(
            rate=self._sample_rate,
            sample_mode=nidaqmx.constants.AcquisitionType.CONTINUOUS,
        )
        task.start()
        self._task = task
        self._samples = []

    def stop_acquisition(self):
        import numpy as np

        if self._task is None:
            raise RuntimeError("start_acquisition() was not called")

        # Read all available samples
        n_available = self._task.in_stream.avail_samp_per_chan
        if n_available > 0:
            data = self._task.read(number_of_samples_per_channel=n_available)
            self._samples.extend(data)

        self._task.stop()
        self._task.close()
        self._task = None

        voltages = np.array(self._samples, dtype=np.float64)
        dt = 1.0 / self._sample_rate
        timestamps = np.arange(len(voltages), dtype=np.float64) * dt
        return timestamps, voltages

    def get_device_info(self) -> dict:
        return {
            "name": self.name,
            "device": self._device,
            "channel": self._channel,
            "sample_rate_hz": self._sample_rate,
        }

    def close(self) -> None:
        if self._task is not None:
            try:
                self._task.stop()
                self._task.close()
            except Exception:
                pass
            self._task = None
