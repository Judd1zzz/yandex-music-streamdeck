from .base import YnisonModel
from typing import List, Optional
from .common import YnisonPlayableItem, YnisonEntityContext, YnisonId, YnisonVersion, YnisonEntityType


class YnisonWaveSource(YnisonModel):
    pass


class YnisonPhonotekaSource(YnisonModel):
    entity_context: YnisonEntityContext
    album_id: Optional[YnisonId] = None
    playlist_id: Optional[YnisonId] = None


class YnisonTrackSource(YnisonModel):
    key: int
    wave_source: Optional[YnisonWaveSource] = None
    phonoteka_source: Optional[YnisonPhonotekaSource] = None


class YnisonWaveEntityOptional(YnisonModel):
    session_id: str


class YnisonEntityOptions(YnisonModel):
    track_sources: Optional[List[YnisonTrackSource]] = None
    wave_entity_optional: Optional[YnisonWaveEntityOptional] = None


class YnisonWaveQueue(YnisonModel):
    recommended_playable_list: Optional[List[YnisonPlayableItem]] = None
    live_playable_index: int = 0
    entity_options: Optional[YnisonEntityOptions] = None


class YnisonQueue(YnisonModel):
    wave_queue: Optional[YnisonWaveQueue] = None


class YnisonQueueOptions(YnisonModel):
    repeat_mode: str = "NONE"


class YnisonPlayerQueue(YnisonModel):
    current_playable_index: int = -1
    entity_id: Optional[str] = None
    entity_type: YnisonEntityType = YnisonEntityType.VARIOUS
    entity_context: YnisonEntityContext = YnisonEntityContext.BASED_ON_ENTITY_BY_DEFAULT
    options: YnisonQueueOptions = YnisonQueueOptions()
    playable_list: List[YnisonPlayableItem] = []
    queue: Optional[YnisonQueue] = None
    from_optional: Optional[str] = None
    version: Optional[YnisonVersion] = None
