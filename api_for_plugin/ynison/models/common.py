import time
from enum import Enum
from pydantic import Field
from .base import YnisonModel
from typing import Optional, Union


class YnisonId(YnisonModel):
    id: str


class YnisonEntityType(str, Enum):
    UNSPECIFIED = "UNSPECIFIED"
    ALBUM = "ALBUM"
    ARTIST = "ARTIST"
    VARIOUS = "VARIOUS"
    RADIO = "RADIO"
    GENERATIVE = "GENERATIVE"
    FM_RADIO = "FM_RADIO"
    VIDEO_WAVE = "VIDEO_WAVE"
    LOCAL_TRACKS = "LOCAL_TRACKS"
    PLAYLIST = "PLAYLIST"

    @classmethod
    def _missing_(cls, value):
        if isinstance(value, str):
            for member in cls:
                if member.value.upper() == value.upper():
                    return member
        return None


class YnisonEntityContext(str, Enum):
    BASED_ON_ENTITY_BY_DEFAULT = "BASED_ON_ENTITY_BY_DEFAULT"

    @classmethod
    def _missing_(cls, value):
        if isinstance(value, str):
             for member in cls:
                 if member.value.upper() == value.upper():
                     return member
        return None


class YnisonPlayableItemType(str, Enum):
    TRACK = "TRACK"

    @classmethod
    def _missing_(cls, value):
        if isinstance(value, str):
            if value.upper() == "TRACK":
                return cls.TRACK
        return None


class YnisonKeepAliveParams(YnisonModel):
    keep_alive_time_seconds: int
    keep_alive_timeout_seconds: int


def generate_version() -> str:
    return str(int(time.time() * 1000))


def get_timestamp() -> int:
    return int(time.time() * 1000)


class YnisonVersion(YnisonModel):
    device_id: str
    version: Union[str, int] = Field(default_factory=generate_version)
    timestamp_ms: int = Field(default_factory=get_timestamp)


class YnisonTrackInfo(YnisonModel):
    track_source_key: int


class YnisonPlayableItem(YnisonModel):
    album_id_optional: Optional[str] = None
    cover_url_optional: Optional[str] = None
    from_: str = Field(alias="from")
    playable_id: str
    playable_type: YnisonPlayableItemType
    title: Optional[str] = None
    track_info: Optional[YnisonTrackInfo] = None
    playback_action_id_optional: Optional[str] = None
    navigation_id_optional: Optional[str] = None
