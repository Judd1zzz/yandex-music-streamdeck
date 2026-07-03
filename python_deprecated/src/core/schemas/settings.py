from dataclasses import dataclass
from typing import Literal


ControlMode = Literal["local", "ynison"]


@dataclass(slots=True)
class PluginSettings:
    control_mode: ControlMode = "local"
    display_mode: str = "cover_title_artists"
    play_style: str = "v1"
    prev_style: str = "v1"
    next_style: str = "v1"
    like_style: str = "v1"
    dislike_style: str = "v1"
    progress_mode: str = "stacked"
    volume_style: str = "v1"
    mute_style: str = "v1"
    
    show_cover: bool = True
    show_title: bool = True
    show_artist: bool = True
    
    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        return cls(
            control_mode=data.get("control_mode", "local"),
            display_mode=data.get("display_mode", "cover_title_artists"),
            play_style=data.get("play_style", "v1"),
            prev_style=data.get("prev_style", "v1"),
            next_style=data.get("next_style", "v1"),
            like_style=data.get("like_style", "v1"),
            dislike_style=data.get("dislike_style", "v1"),
            progress_mode=data.get("progress_mode", "stacked"),
            volume_style=data.get("volume_style", "v1"),
            mute_style=data.get("mute_style", "v1"),
            show_cover=data.get("show_cover", True),
            show_title=data.get("show_title", True),
            show_artist=data.get("show_artist", True)
        )

    def to_dict(self):
        from dataclasses import asdict
        return asdict(self)
