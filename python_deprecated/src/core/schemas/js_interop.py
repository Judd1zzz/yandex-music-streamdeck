from enum import Enum


JS_CONTROLLER_NAME = "window._PyYMController"


class JSMethod(str, Enum):
    GET_FULL_STATE = "getFullState"
    PLAY_PAUSE = "playPause"
    NEXT = "next"
    PREV = "prev"
    TOGGLE_LIKE = "toggleLike"
    TOGGLE_DISLIKE = "toggleDislike"
    CHANGE_VOLUME = "changeVolume"

    def __str__(self):
        return self.value


class UpdateType(str, Enum):
    FULL_STATE = "FULL_STATE"
    DELTA = "DELTA"

    def __str__(self):
        return self.value
