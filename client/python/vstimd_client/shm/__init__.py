"""Shared memory with vstimd from the producer's side.

``vstimd_client.shm.input`` publishes an input device — a wheel, treadmill or eye
tracker — for the server to read every frame. Unix only.
"""

from .input import (
    AxisSpec,
    InputDevice,
    InputDeviceReader,
    Semantic,
    TornRead,
)

__all__ = ["AxisSpec", "InputDevice", "InputDeviceReader", "Semantic", "TornRead"]
