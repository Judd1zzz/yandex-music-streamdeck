import os
import time
import json
import asyncio
import logging
import aiohttp
from typing import Dict, Optional, Union
from src.core.types import YnisonCommand


logger = logging.getLogger("Client")


class YandexMusicClient:
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(YandexMusicClient, cls).__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized: return
        
        self.base_api_host = "localhost"
        self.base_api_port = 8000
        self.api_base = f"http://{self.base_api_host}:{self.base_api_port}"
        self.ws_url = f"ws://{self.base_api_host}:{self.base_api_port}/ws"
        
        self.token = os.getenv("YM_TOKEN")
        self.session: Optional[aiohttp.ClientSession] = None
        
        self.running = False
        self._ws_task = None
        
        self.enabled = True 
        self._enabled_event = asyncio.Event() 
        self._token_event = asyncio.Event()
        
        self.current_state = None
        self.current_track_data = None
        self.current_cover_data = None
        self.current_cover_img = None
        self.last_cover_url = None
        
        self.is_ready = False
        self.is_auth_error = False
        self.ws_connected = False
        self.last_state_update_time = 0
        
        self.liked_tracks = set()
        self.disliked_tracks = set()
        
        self.ui_callbacks: Dict[str, callable] = {}
        
        self._initialized = True
        logger.info("Yandex Music Client Initialized (AsyncIO)")

    async def start_async(self):
        """Запускает клиентские таски в текущем event loop + сокет-соединение"""
        if self.running:
            return
        self.running = True
        
        if self.token:
            self._token_event.set()
        if self.enabled:
            self._enabled_event.set()
        
        self._ws_task = asyncio.create_task(self.run_ws_loop())
        logger.info("Client async connection task started.")

    def start(self):
        pass

    def stop(self):
        self.running = False
        if self._ws_task: self._ws_task.cancel()
        if self.session and not self.session.closed:
             asyncio.create_task(self.session.close())

    def set_enabled(self, enabled: bool):
        if self.enabled == enabled:
            return
        self.enabled = enabled
        logger.info(f"Client Enabled state changed to: {enabled}")
        if enabled:
            self._enabled_event.set()
        else:
             self._enabled_event.clear()
             if self.session and not self.session.closed:
                 asyncio.create_task(self._force_disconnect())

    async def _force_disconnect(self):
        if self.session:
            await self.session.close()

    def update_token(self, token):
        if self.token == token:
            return
        self.token = token
        if token:
            self._token_event.set()
        else:
            self._token_event.clear()

    def register_callback(self, action_uuid: str, callback: callable):
        self.ui_callbacks[action_uuid] = callback

    def unregister_callback(self, action_uuid: str):
        if action_uuid in self.ui_callbacks:
            del self.ui_callbacks[action_uuid]
            
    async def _notify_ui(self):
        cbs = list(self.ui_callbacks.values())
        for callback in cbs:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback()
                else:
                    callback()
            except Exception as e:
                logger.error(f"UI update callback failed: {e}")

    async def run_ws_loop(self):
        logger.info(f"Starting API WebSocket Manager (Target: {self.ws_url})")
        while self.running:
            try:
                if not self.enabled:
                    self.ws_connected = False
                    await self._notify_ui()
                    await self._enabled_event.wait()

                if not self.token or self.is_auth_error:
                    await self._token_event.wait()
                
                if not self.session or self.session.closed:
                    self.session = aiohttp.ClientSession()

                headers = {"Authorization": self.token}
                async with self.session.ws_connect(self.ws_url, headers=headers, timeout=10) as ws:
                    self.ws_connected = True
                    async for msg in ws:
                        if not self.running: break
                        match msg.type:
                            case aiohttp.WSMsgType.TEXT:
                                await self.on_api_message(msg.data)
                            case aiohttp.WSMsgType.CLOSED:
                                if msg.extra == 4001:
                                    self.is_auth_error = True
                                    self._token_event.clear()
                                break
                            case aiohttp.WSMsgType.ERROR:
                                break
                            case _:
                                pass
                    self.ws_connected = False
                    self.is_ready = False
                    if not self.is_auth_error and ws.close_code == 4001:
                        self.is_auth_error = True
                        self._token_event.clear()
                    
                    await self._notify_ui()
                        
            except aiohttp.WSServerHandshakeError as e:
                 self.ws_connected = False
                 if e.status in [401, 4001]:
                     self.is_auth_error = True
                     self._token_event.clear()
                 await asyncio.sleep(5)
            except asyncio.CancelledError:
                 break
            except Exception as e:
                 self.ws_connected = False
                 logger.error(f"WS Loop Error: {e}")
                 await asyncio.sleep(5)
            
            if self.running:
                await asyncio.sleep(2)

    async def on_api_message(self, data):
        try:
            if self.is_auth_error:
                self.is_auth_error = False
            state = json.loads(data)
            self.last_state_update_time = time.time()
            if not self.current_state:
                self.current_state = state
            else:
                self.deep_update(self.current_state, state)
            
            await self.process_track_data()
            await self._notify_ui()
        except:
            pass

    def deep_update(self, target, source):
        for k, v in source.items():
            if isinstance(v, dict) and k in target and isinstance(target[k], dict):
                self.deep_update(target[k], v)
            else: target[k] = v
        return target

    async def process_track_data(self):
        if not self.current_state: return
        ps = self.current_state.get("player_state", {})
        queue = ps.get("player_queue", {})
        idx = queue.get("current_playable_index", 0)
        items = queue.get("playable_list", [])
        
        current = items[idx] if items and 0 <= idx < len(items) else None
        
        if current:
            new_id = current.get("playable_id")
            old_id = self.current_track_data.get("playable_id") if self.current_track_data else None
            
            if str(new_id) != str(old_id):
                self.current_track_data = current
                self.current_cover_data = None
                self.current_cover_img = None
                self.last_cover_url = None
                self.is_ready = True
            else:
                 self.deep_update(self.current_track_data, current)
            
            if ae := current.get("artists_enriched"): self.current_track_data["artists"] = ae
            
            cover = current.get("cover_uri_enriched") or current.get("cover_url_optional")
            if cover and cover != self.last_cover_url:
                asyncio.create_task(self.fetch_cover(cover))
            
            if "is_liked" in current:
                if current["is_liked"]:
                    self.liked_tracks.add(new_id)
                else:
                    self.liked_tracks.discard(new_id)
            if "is_disliked" in current:
                 if current["is_disliked"]:
                    self.disliked_tracks.add(new_id)
                 else:
                    self.disliked_tracks.discard(new_id)
        else:
            if self.is_ready:
                self.current_track_data = None
                self.is_ready = False

    async def fetch_cover(self, url):
        if not url:
            return
        url = url.replace('%%', '200x200')
        if not url.startswith("http"): url = f"https://{url}"
        
        try:
            if not self.session: return
            async with self.session.get(url) as resp:
                if resp.status == 200:
                    data = await resp.read()
                    self.current_cover_data = data
                    self.last_cover_url = url
                    
                    import io
                    from PIL import Image
                    try:
                        loop = asyncio.get_running_loop()
                        def process():
                             r = Image.open(io.BytesIO(data)).convert("RGBA")
                             return r.resize((144, 144), Image.LANCZOS)
                        self.current_cover_img = await loop.run_in_executor(None, process)
                    except: self.current_cover_img = None
                    
                    await self._notify_ui()
        except:
            pass

    async def send_command(self, command: Union[YnisonCommand, str]):
        if not self.token or not self.session: return False
        
        endpoint = command.value if isinstance(command, YnisonCommand) else command
        url = f"{self.api_base}/control/{endpoint}"
        
        headers = {"Authorization": f"Bearer {self.token}"}
        try:
            async with self.session.post(url, headers=headers) as resp:
                return resp.status == 200
        except:
            return False

    @property
    def is_paused(self):
        if not self.current_state:
            return True
        return self.current_state.get("player_state", {}).get("status", {}).get("paused", True)


def get_client():
    return YandexMusicClient()
