"""DAQBackend Protocol definition."""

from __future__ import annotations

from typing import Protocol, runtime_checkable

import numpy as np


@runtime_checkable
class DAQBackend(Protocol):
    @property
    def name(self) -> str: ...

    @property
    def sample_rate_hz(self) -> int: ...

    def start_acquisition(self) -> None: ...

    def stop_acquisition(self) -> tuple[np.ndarray, np.ndarray]:
        """Return (timestamps_s, voltages_v) as 1-D float64 arrays."""
        ...

    def get_device_info(self) -> dict: ...

    def close(self) -> None: ...
