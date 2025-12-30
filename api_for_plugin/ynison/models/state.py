from typing import List
from .device import YnisonDeviceFull
from .player_state import YnisonPlayerState
from .messages import YnisonMessage


class YnisonState(YnisonMessage):
    devices: List[YnisonDeviceFull]
    player_state: YnisonPlayerState
    timestamp_ms: float
