import aiohttp
import asyncio
import src.actions
from typing import Any, Dict
from src.core.action import Action
from src.core.logger import Logger
from src.core.schemas.events import *
from src.core.registry import ACTION_REGISTRY
from src.core.mixins.router import RouterMixin
from src.core.event_handlers import PluginEventHandlersMixin

class Plugin(RouterMixin, PluginEventHandlersMixin):
    def __init__(self, port: int, plugin_uuid: str, event: str, info: Dict[str, Any]):
        self.port = port
        self.plugin_uuid = plugin_uuid
        self.register_event = event
        self.info = info
        
        self.active_actions: Dict[str, Action] = {}
        self.global_settings: Any = None
        self.running = False
        self.ws_session = None
        self.ws = None
        
        try:
            from src.core.cdp import get_cdp_controller
            self.cdp = get_cdp_controller()
            
            from src.core.ynison import get_client
            self.client = get_client()
            
        except ImportError as e:
            Logger.error(f"Failed to import subsystems: {e}")

    @property
    def actions(self):
        return self.active_actions

    def cleanup(self):
        self.running = False
        if self.cdp:
            self.cdp.stop()
        if self.client:
            self.client.stop()

    def stop(self):
        self.cleanup()

    async def run(self):
        """
        Основная точка входа. 
    
        Запускает цикл обработки ивентов, устанавливает сокет-соединение со StreamDeck.
        При разрыве соединения со стороны StreamDeck (WS Closed) — завершает работу.
        Если не удается подключиться 5 раз подряд (15 сек) — завершает работу.
        """
        self.running = True
        Logger.info("Plugin Run Loop Started")

        if hasattr(self.cdp, "start_async"):
             asyncio.create_task(self.cdp.start_async())
        else:
             self.cdp.start() 

        url = f"ws://127.0.0.1:{self.port}"
        retries = 0
        max_retries = 3

        while self.running:
            try:
                async with aiohttp.ClientSession() as session:
                    self.ws_session = session
                    Logger.info(f"Connecting to StreamDeck at {url}...")
                    async with session.ws_connect(url) as ws:
                        retries = 0 
                        self.ws = ws
                        await self._on_open(ws)
                        
                        async for msg in ws:
                            match msg.type:
                                case aiohttp.WSMsgType.TEXT:
                                    await self._on_message(msg.data)
                                case aiohttp.WSMsgType.CLOSED:
                                    Logger.info("StreamDeck WS Closed. Shutting down...")
                                    self.running = False  
                                    break
                                case aiohttp.WSMsgType.ERROR:
                                    Logger.error(f"StreamDeck WS Error: {ws.exception()}")
                                    break
                                case _:
                                    Logger.info(f'msg from ws: {msg}')
            except Exception as e:
                Logger.error(f"StreamDeck Connection Error: {e}")
                retries += 1
                if retries >= max_retries:
                    Logger.error(f"Max retries ({max_retries}) reached. Stream Deck host unreachable. Shutting down.")
                    self.running = False
                    break
                
            if self.running:
                Logger.info(f"Reconnecting to StreamDeck in 3s... (Attempt {retries}/{max_retries})")
                await asyncio.sleep(3)
        
        # Cleanup properly before exiting
        self.cleanup()

    async def _on_open(self, ws: aiohttp.ClientWebSocketResponse):
        Logger.info("StreamDeck WebSocket Connected")
        await self.send_json({'event': self.register_event, 'uuid': self.plugin_uuid})
        Logger.info("On opening send register event...")
        await self.send_json({'event': 'getGlobalSettings', 'context': self.plugin_uuid})
        Logger.info("On opening getting settings...")

    async def send_json(self, data: dict):
        if self.ws and not self.ws.closed:
            try:
                await self.ws.send_json(data)
            except Exception as e:
                Logger.warn(f"Failed to send JSON (WS likely closed): {e}")
                self.ws = None

    async def _on_message(self, message: str):
        await self.route_message(message)

    def _handle_new_action(self, obj: WillAppearModel):
        """
        Вызывается RouterMixin, когда событие willAppear получено для неизвестного контекста.
        
        Создает и регистрирует новый экземпляр action на основе его UUID.
        """
        uuid = obj.action
        context = obj.context
        
        if uuid in ACTION_REGISTRY:
            ActionClass = ACTION_REGISTRY[uuid]
            settings = obj.payload.get('settings', {}) if obj.payload else {}
            
            action = ActionClass(
                action=uuid,
                context=context,
                settings=settings,
                plugin=self,
                client=self.client,
                cdp_controller=self.cdp
            )
            self.active_actions[context] = action
            
            if hasattr(self, 'recalculate_ynison_state'):
                 asyncio.create_task(self.recalculate_ynison_state())
                 
            return action
        else:
            Logger.error(f"Unknown Action UUID: {uuid}")
            return None

    async def recalculate_ynison_state(self):
        """
        Проверяет, находится ли какой-либо action в режиме 'ynison'.

        Если да -> гарантирует, что клиент запущен.
        Если нет -> гарантирует, что клиент остановлен (во избежание циклов переподключения с ошибкой 1006).
        """
        needs_ynison = False
        for action in self.active_actions.values():
            if action.settings.get("control_mode") == "ynison":
                needs_ynison = True
                break
        
        if needs_ynison:
            if not self.client.running:
                Logger.info("Starting Ynison Client (Requested by Action)")
                asyncio.create_task(self.client.start_async())
        else:
            if self.client.running:
                Logger.info("Stopping Ynison Client (No Actions require it)")
                await self.client.stop()

    async def on_did_receive_global_settings(self, obj: DidReceiveGlobalSettingsModel):
        self.global_settings = obj.payload.get('settings', {})
        for action in self.active_actions.values():
            if hasattr(action, 'on_did_receive_global_settings'):
                res = action.on_did_receive_global_settings(self.global_settings)
                if asyncio.iscoroutine(res): await res
        
        if "token" in self.global_settings and self.client:
             self.client.update_token(self.global_settings["token"])
             
        await self.recalculate_ynison_state()

    async def on_system_did_wake_up(self, obj: SystemDidWakeUpModel):
        for action in self.active_actions.values():
             if hasattr(action, 'check_health'): action.check_health()
             
    async def on_send_to_plugin(self, obj: SendToPluginModel):
        from src.core.schemas.pi import ApplySettingsPayload
        
        try:
            if obj.payload.get("event") == "applySettingsToAll":
                data = ApplySettingsPayload.model_validate(obj.payload)
                
                new_settings = data.settings
                for act in self.active_actions.values():
                    act.settings.update(new_settings)
                    fake_obj = DidReceiveSettingsModel(
                        event="didReceiveSettings",
                        context=act.context,
                        action=act.action_uuid,
                        device=obj.device,
                        payload={"settings": act.settings, "coordinates": {}}
                    )
                    res = act.on_did_receive_settings(fake_obj)
                    if asyncio.iscoroutine(res): await res
                    await self.set_settings(act.context, act.settings)
                
                await self.recalculate_ynison_state()
        except Exception as e:
            Logger.warn(f"on_send_to_plugin parse error or ignore: {e}")

    async def set_settings(self, context: str, payload: dict):
        await self.send_json({
            'event': 'setSettings',
            'context': context,
            'payload': payload
        })
