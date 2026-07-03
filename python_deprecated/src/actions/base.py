import asyncio
from src.core.action import Action
from src.core.logger import Logger
from src.core.types import HealthStatus
from src.actions.mixins import TrackObservable
from src.core.pi_communicator import PIMessenger
from src.core.settings_manager import SettingsProxy
from src.core.schemas.settings import PluginSettings
from src.core.mixins.task import BackgroundTaskMixin
from src.core.event_handlers import ActionEventHandlersMixin
from src.core.schemas.pi import TokenStatusEnum, LocalStatusEnum
from src.core.schemas.events import (
    EventType,
    DidReceiveSettingsModel,
    WillAppearModel,
    WillDisappearModel,
    PropertyInspectorDidAppearModel
)


class YandexMusicBaseAction(Action, ActionEventHandlersMixin, BackgroundTaskMixin):
    def __init__(self, action: str, context: str, settings: dict, plugin, client=None, cdp_controller=None):
        super().__init__(action, context, settings, plugin)
        self.client = client
        self.cdp = cdp_controller
        self.pi = PIMessenger(self)
        
        _cfg = PluginSettings.from_dict(settings)
        self.cfg = SettingsProxy(_cfg, self._auto_save_settings)
        
        if self.plugin.global_settings and "token" in self.plugin.global_settings:
            try:
                self.client.update_token(self.plugin.global_settings["token"])
            except: pass
        

    async def _auto_save_settings(self, settings_obj: PluginSettings):
        """
        Колбэк, вызываемый при изменении любого параметра настроек.
        Автоматически сохраняет обновленные настройки.
        """
        loop = asyncio.get_running_loop()
        path = settings_obj.to_dict()
        
        if loop.is_running():
             asyncio.create_task(self.plugin.set_settings(self.context, path))

    def get_mode(self):
        return self.cfg.control_mode

    async def _register_based_on_mode(self):
        self.client.unregister_callback(self.context)
        
        mode = self.get_mode()
        if mode == "local":
            self.cdp.register_observer(self.on_local_update, events=self.subscribe_events)
            asyncio.create_task(self.render())
        else:
            self.client.register_callback(self.context, self.render)
            asyncio.create_task(self.render())

    @property
    def subscribe_events(self) -> set[EventType]:
        return {EventType.CONNECTION}

    async def on_did_receive_settings(self, obj: DidReceiveSettingsModel):
        settings = obj.payload.get('settings', {})
        old_mode = self.get_mode()
        self.settings = settings
        
        _cfg = PluginSettings.from_dict(settings)
        self.cfg = SettingsProxy(_cfg, self._auto_save_settings)

        new_mode = self.get_mode()
        
        if old_mode != new_mode:
             Logger.info(f"[{self.context}] Mode switched: {old_mode} -> {new_mode}")
             await self._register_based_on_mode()
        
        await self.render()
             
    async def on_did_receive_global_settings(self, settings: dict):
        if settings and "token" in settings:
            try:
                self.client.update_token(settings["token"])
            except:
                pass

    async def on_local_update(self, event_type: EventType, data):
        """
        Обрабатывает входящие апдейты от LocalController.
        Маршрутизирует события (трек, воспроизведение, громкость) в соответствующие методы.
        """
        if self.get_mode() != "local":
            return
        
        match event_type:
            case EventType.TRACK_INFO:
                await self.on_track_update(data)
            case EventType.PLAYBACK:
                await self.on_playback_update(data)
            case EventType.LIKE:
                await self.on_like_update(data)
            case EventType.DISLIKE:
                await self.on_dislike_update(data)
            case EventType.VOLUME:
                await self.on_volume_update(data)
            case EventType.CONNECTION:
                await self.render()

    # дэфолтные хэндлеры по умолчанию просто отображаются, если они переопределены (по умолчанию база прослушивает только соединение)
    async def on_track_update(self, data): await self.render()
    async def on_playback_update(self, data): pass
    async def on_like_update(self, data): pass
    async def on_dislike_update(self, data): pass
    async def on_volume_update(self, data): pass

    def should_ignore_errors(self):
        return self.get_mode() == "local"

    def check_health(self) -> HealthStatus:
        mode = self.get_mode()
        if mode == "local":
            return HealthStatus.OK if self.cdp.is_connected else HealthStatus.DISCONNECTED_LOCAL
        else:
            if self.client.is_auth_error:
                return HealthStatus.AUTH_ERROR
            if not self.client.token:
                return HealthStatus.NO_TOKEN
            if not self.client.ws_connected:
                return HealthStatus.OFFLINE
            return HealthStatus.OK

    async def render(self):
        """Отрисовывает текущее состояние действия и отправляет статус соединения в Property Inspector."""
        await self.render_action()
        health = self.check_health()
        if self.get_mode() == "ynison":
            match health:
                case HealthStatus.AUTH_ERROR:
                    if not self.should_ignore_errors(): await self.show_alert()
                    await self.pi.send_token_status(TokenStatusEnum.INVALID)
                case HealthStatus.NO_TOKEN:
                    if not self.should_ignore_errors(): await self.show_alert()
                    await self.pi.send_token_status(TokenStatusEnum.MISSING)
                case HealthStatus.OFFLINE:
                    await self.pi.send_token_status(TokenStatusEnum.OFFLINE)
                case _:
                    await self.pi.send_token_status(TokenStatusEnum.VALID)

        elif self.get_mode() == "local":
             status = LocalStatusEnum.CONNECTED if health == HealthStatus.OK else LocalStatusEnum.DISCONNECTED
             await self.pi.send_local_status(status)



    async def render_action(self):
        """
        подклассы должны вызывать этот метод или переопределить свой.
        обеспечивает стандартное отображение 'информации о треке', позволяющее показывать обложку и текст
        """
        data = {}
        if self.get_mode() == "local":
            info = self.cdp.track_info
            data = {
                "title": info.title,
                "artist": info.artist,
                "cover_url": info.cover_url
            }
        else:
            if self.client.current_track_data:
                artists = self.client.current_track_data.get("artists", [])
                artist_str = ", ".join([a.get("name") for a in artists])
                data = {
                    "title": self.client.current_track_data.get("title", ""),
                    "artist": artist_str,
                    "cover_url": self.client.last_cover_url
                }

        await self.render_optimized(track_data=data, icon_name=self.get_icon_name())
    
    def get_icon_name(self):
        """Подклассы могут переопределять, чтобы предоставить накладной значок (например, pause.png)"""
        return None

    async def on_will_appear(self, obj: WillAppearModel):
        await self._register_based_on_mode() 
        await self.render()

    async def on_will_disappear(self, obj: WillDisappearModel):
        self.cancel_all_tasks()
        self.client.unregister_callback(self.context)
        self.cdp.unregister_observer(self.on_local_update)

    async def on_property_inspector_did_appear(self, obj: PropertyInspectorDidAppearModel):
        if self.get_mode() == "local":
             status = LocalStatusEnum.CONNECTED if self.cdp.is_connected else LocalStatusEnum.DISCONNECTED
             await self.pi.send_local_status(status)
        else:
            if self.client.is_auth_error:
                await self.pi.send_token_status(TokenStatusEnum.INVALID)
            elif not self.client.token:
                await self.pi.send_token_status(TokenStatusEnum.MISSING)
            else:
                await self.pi.send_token_status(TokenStatusEnum.VALID)


class YandexMusicTrackAction(TrackObservable, YandexMusicBaseAction):
    """
    Дефолтный action, требующий обновления информации о треке (обложка, название, исполнитель).
    """
    pass
