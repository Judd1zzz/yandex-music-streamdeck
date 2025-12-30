from typing import Optional
from .base import YnisonModel
from .common import YnisonVersion, YnisonModel, YnisonId


class YnisonDeviceInfo(YnisonModel):
    device_id: str
    type: str
    title: str
    app_name: str
    app_version: str


class YnisonDeviceCapabilities(YnisonModel):
    can_be_player: bool
    can_be_remote_controller: bool
    volume_granularity: int


class YnisonDeviceVolumeInfo(YnisonModel):
    volume: float
    version: Optional[YnisonVersion] = None


class YnisonDevice(YnisonModel):
    info: YnisonDeviceInfo
    capabilities: YnisonDeviceCapabilities = YnisonDeviceCapabilities(can_be_player=False, can_be_remote_controller=False, volume_granularity=0)
    volume_info: YnisonDeviceVolumeInfo = YnisonDeviceVolumeInfo(volume=0)
    is_shadow: bool = False


class YnisonSession(YnisonId):
    pass


class YnisonDeviceFull(YnisonDevice):
    session: Optional[YnisonSession] = None
    volume: float
    is_offline: Optional[bool] = None
