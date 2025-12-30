from enum import Enum
from typing import Literal
from pydantic import BaseModel, Field



class TokenStatusEnum(str, Enum):
    VALID = "valid"
    INVALID = "invalid"
    MISSING = "missing"
    OFFLINE = "offline"

class LocalStatusEnum(str, Enum):
    CONNECTED = "connected"
    DISCONNECTED = "disconnected"
    LOADING = "loading"


class BasePIEvent(BaseModel):
    event: str

class TokenStatusEvent(BasePIEvent):
    event: Literal["TokenStatus"] = "TokenStatus"
    status: TokenStatusEnum

class LocalStatusEvent(BasePIEvent):
    event: Literal["LocalStatus"] = "LocalStatus"
    status: LocalStatusEnum


class ApplySettingsPayload(BaseModel):
    event: Literal["applySettingsToAll"] = "applySettingsToAll"
    settings: dict = Field(default_factory=dict)
