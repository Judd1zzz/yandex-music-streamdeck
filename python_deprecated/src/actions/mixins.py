from src.core.schemas.events import EventType


class TrackObservable:
    @property
    def subscribe_events(self):
        return super().subscribe_events | {EventType.TRACK_INFO}

class PlaybackObservable:
    @property
    def subscribe_events(self):
        return super().subscribe_events | {EventType.PLAYBACK}

class LikeObservable:
    @property
    def subscribe_events(self):
        return super().subscribe_events | {EventType.LIKE}

class DislikeObservable:
    @property
    def subscribe_events(self):
        return super().subscribe_events | {EventType.DISLIKE}

class VolumeObservable:
    @property
    def subscribe_events(self):
        return super().subscribe_events | {EventType.VOLUME}
