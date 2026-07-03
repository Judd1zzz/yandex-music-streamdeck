from enum import Enum


class YnisonCommand(str, Enum):
    PLAY_PAUSE = "play_pause"
    NEXT = "next"
    PREV = "prev"
    LIKE = "like"
    DISLIKE = "dislike"
    VOLUME_UP = "volume_up"
    VOLUME_DOWN = "volume_down"


class HealthStatus(str, Enum):
    OK = "ok"
    AUTH_ERROR = "auth_error"
    NO_TOKEN = "no_token"
    OFFLINE = "offline"
    DISCONNECTED_LOCAL = "disconnected_local"
