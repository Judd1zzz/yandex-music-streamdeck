from typing import Optional, Any, Dict
from enum import Enum
from pydantic import BaseModel, Field


class StreamDeckEvent(str, Enum):
    KEY_DOWN = "keyDown"
    KEY_UP = "keyUp"
    DID_RECEIVE_GLOBAL_SETTINGS = "didReceiveGlobalSettings"
    DID_RECEIVE_SETTINGS = "didReceiveSettings"
    WILL_APPEAR = "willAppear"
    WILL_DISAPPEAR = "willDisappear"
    PROPERTY_INSPECTOR_DID_APPEAR = "propertyInspectorDidAppear"
    SEND_TO_PLUGIN = "sendToPlugin"
    SYSTEM_DID_WAKE_UP = "systemDidWakeUp"
    APPLICATION_DID_LAUNCH = "applicationDidLaunch"
    APPLICATION_DID_TERMINATE = "applicationDidTerminate"
    TITLE_PARAMETERS_DID_CHANGE = "titleParametersDidChange"

    def __str__(self):
        return self.value


class BaseEventModel(BaseModel):
    event: str
    context: Optional[str] = None
    device: Optional[str] = None
    action: Optional[str] = None
    payload: Optional[Dict[str, Any]] = None

class KeyDownModel(BaseEventModel):
    event: str = StreamDeckEvent.KEY_DOWN

class KeyUpModel(BaseEventModel):
    event: str = StreamDeckEvent.KEY_UP

class WillAppearModel(BaseEventModel):
    event: str = StreamDeckEvent.WILL_APPEAR

class WillDisappearModel(BaseEventModel):
    event: str = StreamDeckEvent.WILL_DISAPPEAR

class DidReceiveSettingsModel(BaseEventModel):
    event: str = StreamDeckEvent.DID_RECEIVE_SETTINGS

class DidReceiveGlobalSettingsModel(BaseEventModel):
    event: str = StreamDeckEvent.DID_RECEIVE_GLOBAL_SETTINGS

class PropertyInspectorDidAppearModel(BaseEventModel):
    event: str = StreamDeckEvent.PROPERTY_INSPECTOR_DID_APPEAR

class SendToPluginModel(BaseEventModel):
    event: str = StreamDeckEvent.SEND_TO_PLUGIN

class SystemDidWakeUpModel(BaseEventModel):
    event: str = StreamDeckEvent.SYSTEM_DID_WAKE_UP

class ApplicationDidLaunchModel(BaseEventModel):
    event: str = StreamDeckEvent.APPLICATION_DID_LAUNCH

class ApplicationDidTerminateModel(BaseEventModel):
    event: str = StreamDeckEvent.APPLICATION_DID_TERMINATE

class TitleParametersDidChangeModel(BaseEventModel):
    event: str = StreamDeckEvent.TITLE_PARAMETERS_DID_CHANGE

class EventType(str, Enum):
    CONNECTION = "connection"
    TRACK_INFO = "track_info"
    PLAYBACK = "playback"
    LIKE = "like"
    DISLIKE = "dislike"
    VOLUME = "volume"
