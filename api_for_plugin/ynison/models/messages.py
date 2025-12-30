import uuid
import time
from enum import Enum
from pydantic import Field
from .base import YnisonModel
from typing import Optional, Union
from .player_state import YnisonPlayerState
from .device import YnisonDevice, YnisonDeviceFull


class YnisonMessageType(str, Enum):
    REDIRECT = "Redirect"
    STATE = "State"
    ERROR = "Error"


class YnisonErrorDetails(YnisonModel):
    ynison_error_code: Optional[str] = None
    ynison_backoff_millis: Optional[str] = None


class YnisonError(YnisonModel):
    details: Optional[YnisonErrorDetails] = None
    grpc_code: int
    http_code: int
    http_status: str
    message: Optional[str] = None


class YnisonErrorMessage(YnisonModel):
    error: YnisonError


class YnisonMessage(YnisonModel):
    rid: str = Field(default_factory=lambda: str(uuid.uuid4()))


def get_current_timestamp_ms() -> int:
    return int(time.time() * 1000)


class YnisonUpdateMessage(YnisonMessage):
    activity_interception_type: str = "DO_NOT_INTERCEPT_BY_DEFAULT"
    player_action_timestamp_ms: int = Field(default_factory=get_current_timestamp_ms)


class YnisonUpdatePlayerStateMessage(YnisonUpdateMessage):
    update_player_state: YnisonPlayerState


class YnisonFullState(YnisonModel):
    player_state: YnisonPlayerState
    device: Union[YnisonDeviceFull, YnisonDevice]
    is_currently_active: bool


class YnisonUpdateFullStateMessage(YnisonUpdateMessage):
    update_full_state: YnisonFullState
