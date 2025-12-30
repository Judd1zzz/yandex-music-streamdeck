from typing import Optional
from .base import YnisonModel
from .common import YnisonVersion
from .queue import YnisonPlayerQueue


class YnisonPlayerStateStatus(YnisonModel):
    duration_ms: int
    paused: bool = True
    playback_speed: float = 1
    progress_ms: int
    version: Optional[YnisonVersion] = None


class YnisonPlayerState(YnisonModel):
    player_queue: YnisonPlayerQueue
    status: YnisonPlayerStateStatus
