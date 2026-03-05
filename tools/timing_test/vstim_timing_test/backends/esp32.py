"""ESP32 backend stub — not yet implemented."""

from __future__ import annotations


class ESP32Backend:
    """Placeholder for a future ESP32-based photodiode reader.

    Raises NotImplementedError on all method calls.
    """

    @property
    def name(self) -> str:
        return "ESP32Backend"

    @property
    def sample_rate_hz(self) -> int:
        raise NotImplementedError("ESP32Backend is not implemented yet")

    def start_acquisition(self) -> None:
        raise NotImplementedError("ESP32Backend is not implemented yet")

    def stop_acquisition(self):
        raise NotImplementedError("ESP32Backend is not implemented yet")

    def get_device_info(self) -> dict:
        raise NotImplementedError("ESP32Backend is not implemented yet")

    def close(self) -> None:
        pass
