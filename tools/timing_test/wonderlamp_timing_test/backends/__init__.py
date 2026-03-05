"""DAQ backend registry and auto-detection.

Auto-detection probes in order:
  NI-DAQmx → LabJack T4/T7 → LabJack U3 → SimulatedBackend

Use ``auto_detect()`` to get the best available backend at runtime.
"""

from __future__ import annotations

from .base import DAQBackend
from .simulation import SimulatedBackend


def auto_detect(prefer: str = "auto") -> DAQBackend:
    """Return the best available DAQ backend.

    Parameters
    ----------
    prefer:
        ``"auto"`` tries hardware in priority order and falls back to
        simulated.  Pass ``"ni"``, ``"t4"``, ``"u3"``, or ``"simulated"``
        to force a specific backend.
    """
    if prefer == "simulated":
        return SimulatedBackend()

    if prefer in ("auto", "ni"):
        try:
            import nidaqmx  # type: ignore[import]
            from .ni import NIBackend
            return NIBackend()
        except (ImportError, Exception):
            if prefer == "ni":
                raise RuntimeError("NI-DAQmx backend not available") from None

    if prefer in ("auto", "t4"):
        try:
            from labjack import ljm  # type: ignore[import]
            from .labjack_t4 import LabJackT4Backend
            return LabJackT4Backend()
        except (ImportError, Exception):
            if prefer == "t4":
                raise RuntimeError("LabJack T4/T7 backend not available") from None

    if prefer in ("auto", "u3"):
        try:
            import u3  # type: ignore[import]
            from .labjack_u3 import LabJackU3Backend
            return LabJackU3Backend()
        except (ImportError, Exception):
            if prefer == "u3":
                raise RuntimeError("LabJack U3 backend not available") from None

    return SimulatedBackend()


__all__ = ["DAQBackend", "SimulatedBackend", "auto_detect"]
