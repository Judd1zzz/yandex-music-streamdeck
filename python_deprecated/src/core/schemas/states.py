from typing import Optional
from dataclasses import dataclass, field


@dataclass(slots=True)
class TrackData:
    title: str = ""
    artist: str = ""
    cover_url: str = ""  
    track_id: str = "" 

    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        return cls(
            title=data.get("title", ""),
            artist=data.get("artist", ""),
            cover_url=data.get("cover", ""),
            track_id=str(data.get("id", ""))
        )

@dataclass(slots=True)
class PlaybackData:
    is_playing: bool = False
    current_sec: float = 0.0
    total_sec: float = 0.0
    progress: float = 0.0 
    timestamp: float = 0.0
    
    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        return cls(
            is_playing=data.get("playing", False),
            current_sec=data.get("now_sec") or 0.0,
            total_sec=data.get("total_sec") or 0.0,
            progress=data.get("ratio") or 0.0,
            timestamp=0.0 
        )

@dataclass(slots=True)
class LikeData:
    is_liked: bool = False
    
    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        return cls(is_liked=data.get("liked", False))

@dataclass(slots=True)
class DislikeData:
    is_disliked: bool = False

    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        return cls(is_disliked=data.get("disliked", False))

@dataclass(slots=True)
class VolumeData:
    current: float = 0.0
    is_muted: bool = False
    
    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls()
        vol = data.get("current") or 0.0
        return cls(
            current=vol,
            is_muted=data.get("is_muted", False)
        )

@dataclass(slots=True)
class MediaState:
    track: TrackData = field(default_factory=TrackData)
    playback: PlaybackData = field(default_factory=PlaybackData)
    like: LikeData = field(default_factory=LikeData)
    dislike: DislikeData = field(default_factory=DislikeData)
    volume: VolumeData = field(default_factory=VolumeData)

    @classmethod
    def from_dict(cls, data: dict):
        """Парсит полную структуру json из ответа getFullState"""
        if not data: return cls()
        
        track_obj = TrackData.from_dict(data.get("track", {}))
        
        st = data.get("state", {})
        prog = data.get("progress", {})
        
        playback_obj = PlaybackData(
            is_playing=st.get("playing", False),
            current_sec=prog.get("now_sec") or 0.0,
            total_sec=prog.get("total_sec") or 0.0,
            progress=prog.get("ratio") or 0.0,
            timestamp=0.0
        )
        
        like_obj = LikeData(is_liked=st.get("liked", False))
        dislike_obj = DislikeData(is_disliked=st.get("disliked", False))
        volume_obj = VolumeData.from_dict(data.get("volume", {}))
        
        return cls(
            track=track_obj,
            playback=playback_obj,
            like=like_obj,
            dislike=dislike_obj,
            volume=volume_obj
        )


@dataclass(slots=True)
class ActionResultData:
    success: bool = False
    error: Optional[str] = None
    action: Optional[str] = None
    new_state: Optional[bool] = None
    is_playing: Optional[bool] = None
    volume: Optional[float] = None
    is_muted: Optional[bool] = None
    
    @classmethod
    def from_dict(cls, data: dict):
        if not data: return cls(success=False, error="No data")
        return cls(
            success=data.get("success", False),
            error=data.get("error"),
            action=data.get("action"),
            new_state=data.get("new_state") if "new_state" in data else data.get("is_disliked"),
            is_playing=data.get("is_playing"),
            volume=data.get("volume"),
            is_muted=data.get("is_muted")
        )
