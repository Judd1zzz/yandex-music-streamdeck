import time
import aiohttp
import asyncio
from pathlib import Path
from src.core.logger import Logger
from collections import defaultdict
from src.core.schemas.events import EventType
from src.core.schemas.js_interop import JS_CONTROLLER_NAME, JSMethod, UpdateType
from src.core.schemas.states import MediaState, ActionResultData, TrackData, PlaybackData


logger = Logger


class CDPMediaController:
    """
    Управляет локальным клиентом Яндекс Музыки через CDP протокол.
    Нативная реализация на asyncio.
    """
    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super(CDPMediaController, cls).__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return
            
        self.port = 9222
        self._initialized = True
        self._load_scripts()
        
        self.is_connected = False
        self.observers = defaultdict(list)  # Dict[EventType, List[Callable]]
        
        self._reader_task = None
        self._connection_monitor_task = None
        self.running = False
        
        self.cdp_session = None
        self.cdp_ws = None
        self._connect_lock = asyncio.Lock()
        
        self._pending_futures = {}  # id -> future
        self._current_cmd_id = 1
        
        self.last_state = MediaState()
        
        self._playback_optimistic_until = 0
        self._volume_optimistic_until = 0

    def _load_scripts(self):
        """Подгружает содержимое js-скрипта из файловой системы"""
        try:
            current_dir = Path(__file__).parent
            script_path = current_dir / "scripts" / "injected_api.js"
            with open(script_path, "r", encoding="utf-8") as f:
                self._js_payload = f.read()
        except Exception as e:
            logger.error(f"CRITICAL: Failed to load JS API: {e}")
            self._js_payload = ""

    async def _ensure_injection(self):
        """
        Проверяет наличие объекта window._PyYMController на странице.
        Если объект отсутствует — выполняет инъекцию js-скрипта.
        """
        if not self.is_connected: return False

        # очень легкая проверка (возвращает true/false)
        check_script = f"!!({JS_CONTROLLER_NAME})"
        is_injected = await self.execute_script(check_script)

        if not is_injected:
            logger.info("Injecting Yandex API JS...")
            await self.execute_script(self._js_payload)
            await asyncio.sleep(0.1) 
        
        return True

    async def start_async(self):
        if self.running: return
        self.running = True
        self._connection_monitor_task = asyncio.create_task(self._connection_loop())
        logger.info("LocalController started (Event-Driven Mode).")

    def start(self): pass 
    def stop(self):
        self.running = False
        if self._connection_monitor_task: self._connection_monitor_task.cancel()
        if self._reader_task: self._reader_task.cancel()

    def register_observer(self, callback, events: set[EventType] = None):
        if events is None: events = {EventType.CONNECTION}
        for event in events:
            if callback not in self.observers[event]:
                self.observers[event].append(callback)

    def unregister_observer(self, callback):
        for event_type in list(self.observers.keys()):
            if callback in self.observers[event_type]:
                self.observers[event_type].remove(callback)

    def _set_connection_status(self, connected: bool):
        if self.is_connected != connected:
            self.is_connected = connected
            self._notify_observers(EventType.CONNECTION, {"connected": connected})
            logger.info(f"Local Connection Status Changed: {connected}")

    def _notify_observers(self, event_type: EventType, data):
        if event_type in self.observers:
            for callback in self.observers[event_type]:
                try:
                    if asyncio.iscoroutinefunction(callback):
                        asyncio.create_task(callback(event_type, data))
                    else:
                        callback(event_type, data)
                except Exception as e:
                    logger.error(f"Observer callback failed: {e}")

    async def _connection_loop(self):
        """
        Цикл поддержания соединения и контроля окружения.
        
        Обеспечивает автоматическое переподключение и повторную инъекцию скриптов
        (например, при перезагрузке плагина).
        """
        while self.running:
            try:
                if not self.is_connected or not self.cdp_ws or self.cdp_ws.closed:
                     if await self._connect():
                         await self._setup_cdp_environment()
                else:
                     await self._ensure_injection()
                     
                await asyncio.sleep(2.0)
            except Exception as e:
                logger.error(f"Connection loop error: {e}")
                await asyncio.sleep(2.0)

    async def _connect(self):
        ws_url = await self._get_cdp_ws_url()
        if not ws_url:
            self._set_connection_status(False)
            return False
        try:
            if not self.cdp_session or self.cdp_session.closed:
                 self.cdp_session = aiohttp.ClientSession()
            
            self.cdp_ws = await self.cdp_session.ws_connect(ws_url, timeout=5.0)
            
            if self._reader_task:
                self._reader_task.cancel()
            self._reader_task = asyncio.create_task(self._ws_reader())
            
            self._set_connection_status(True)
            return True
        except:
            self.cdp_ws = None
            self._set_connection_status(False)
            return False

    async def _setup_cdp_environment(self):
        """
        Инициализирует окружение CDP после подключения.
        
        Включает Runtime, регистрирует sdNotify для обратных вызовов
        и запускает 'наблюдатель' на странице.
        """
        try:
            await self._send_rpc("Runtime.enable", {})
            await self._send_rpc("Runtime.addBinding", {"name": "sdNotify"})
            await self._ensure_injection()
            
            start_script = f"{JS_CONTROLLER_NAME}.startObservation()"
            await self.execute_script(start_script)

            initial_state = await self.fetch_state()
            if initial_state.track.title: # костыльная проверка на валидность
                 self.last_state = initial_state
                 self._update_full_state()
            
        except Exception as e:
            logger.error(f"Setup Env Failed: {e}")

    async def _ws_reader(self):
        """
        Чтение сообщений WebSocket.
        Маршрутизирование входящих данных: ответы на RPC-запросы и события от Runtime.binding.
        """
        try:
            async for msg in self.cdp_ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    try:
                        import json
                        data = json.loads(msg.data)
                        
                        if "id" in data:
                            cmd_id = data["id"]
                            if cmd_id in self._pending_futures:
                                fut = self._pending_futures.pop(cmd_id)
                                if not fut.done():
                                    if "error" in data:
                                        fut.set_exception(Exception(data["error"]["message"]))
                                    else:
                                        fut.set_result(data.get("result", {}))
                        
                        elif "method" in data:
                            await self._handle_event(data["method"], data.get("params", {}))
                            
                    except Exception as e:
                         logger.error(f"WS Parse Error: {e}")
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    break
        except Exception as e:
            logger.error(f"WS Reader Error: {e}")
        finally:
            self._set_connection_status(False)

    async def _handle_event(self, method, params):
        if method == "Runtime.bindingCalled":
            if params.get("name") == "sdNotify":
                try:
                    import json
                    payload = json.loads(params.get("payload", "{}"))
                    await self._handle_notify_payload(payload)
                except Exception as e:
                    logger.error(f"Binding payload error: {e}")

    async def _handle_notify_payload(self, data: dict):
        msg_type = data.get("type")
        payload = data.get("payload", {})
        
        match msg_type:
            case UpdateType.FULL_STATE:
                new_state = MediaState.from_dict(payload)
                self.last_state = new_state
                self._update_full_state()
            case UpdateType.DELTA:
                self._apply_delta(payload)

    def _update_full_state(self):
        self._notify_observers(EventType.CONNECTION, {"connected": True})
        self._notify_observers(EventType.TRACK_INFO, self.last_state.track)
        self._notify_observers(EventType.PLAYBACK, self.last_state.playback)
        self._notify_observers(EventType.VOLUME, self.last_state.volume)
        self._notify_observers(EventType.LIKE, self.last_state.like)
        self._notify_observers(EventType.DISLIKE, self.last_state.dislike)

    def _apply_delta(self, delta: dict):
        """
        Применяет частичные обновления (delta) к текущему стейту.
        Обрабатывает только измененные поля и уведомляет наблюдателей.
        """
        # Logger.info(f'update from client: {delta}')  # для отладки
        if "track" in delta:
            track_delta = delta["track"]
            from dataclasses import asdict
            
            current_track_dict = asdict(self.last_state.track)
            if "id" in track_delta: current_track_dict["track_id"] = str(track_delta["id"])
            if "title" in track_delta: current_track_dict["title"] = track_delta["title"]
            if "artist" in track_delta: current_track_dict["artist"] = track_delta["artist"]
            if "cover" in track_delta: current_track_dict["cover_url"] = track_delta["cover"]
            
            new_track = TrackData(**current_track_dict)
            if new_track != self.last_state.track:
                self.last_state.track = new_track
                self._notify_observers(EventType.TRACK_INFO, new_track)

        if "state" in delta:
            st = delta["state"]
            if "liked" in st:
                self.last_state.like.is_liked = st["liked"]
                self._notify_observers(EventType.LIKE, self.last_state.like)
            
            if "disliked" in st:
                self.last_state.dislike.is_disliked = st["disliked"]
                self._notify_observers(EventType.DISLIKE, self.last_state.dislike)
                
            if "playing" in st:
                self.last_state.playback.is_playing = st["playing"]
                self._notify_observers(EventType.PLAYBACK, self.last_state.playback)

        if "progress" in delta:
            prog = delta["progress"]
            pb = self.last_state.playback
            if "now_sec" in prog: pb.current_sec = float(prog["now_sec"] or 0.0)
            if "total_sec" in prog: pb.total_sec = float(prog["total_sec"] or 0.0)
            if "ratio" in prog: pb.progress = float(prog["ratio"] or 0.0)
            pb.timestamp = time.time()
            
            if time.time() > self._playback_optimistic_until:
                 self._notify_observers(EventType.PLAYBACK, pb)

        if "volume" in delta:
            vol = delta["volume"]
            v_obj = self.last_state.volume
            should_notify = False
            
            if "current" in vol: 
                v_obj.current = float(vol["current"] or 0.0)
                should_notify = True
            if "is_muted" in vol: 
                v_obj.is_muted = vol["is_muted"]
                should_notify = True
                
            if should_notify and time.time() > self._volume_optimistic_until:
                 self._notify_observers(EventType.VOLUME, v_obj)

    async def _send_rpc(self, method, params):
        """
        Отправляет RPC-команду через WebSocket и ожидает результат.
        Использует Future для сопоставления асинхронного ответа с запросом.
        """
        if not self.cdp_ws or self.cdp_ws.closed: raise Exception("No Connection")
        
        cmd_id = self._current_cmd_id
        self._current_cmd_id += 1
        
        loop = asyncio.get_running_loop()
        future = loop.create_future()
        self._pending_futures[cmd_id] = future
        
        payload = {"id": cmd_id, "method": method, "params": params}
        await self.cdp_ws.send_json(payload)
        
        return await asyncio.wait_for(future, timeout=5.0)

    async def execute_script(self, script):
        if not await self.ensure_connection(): return False
        try:
            res = await self._send_rpc("Runtime.evaluate", {
                "expression": script, 
                "awaitPromise": True, 
                "returnByValue": True
            })
            
            if "result" in res:
                 return res["result"].get("value")
            return False
        except Exception as e:
            logger.error(f"CDP Exec Error: {e}")
            return False

    async def _get_cdp_ws_url(self):
        try:
            async with aiohttp.ClientSession() as session:
                url = f"http://127.0.0.1:{self.port}/json/list"
                async with session.get(url, timeout=2.0) as resp:
                    if resp.status == 200:
                        pages = await resp.json()
                        for page in pages:
                            url_str = page.get("url", "")
                            title_str = page.get("title", "")
                            if "music.yandex" in url_str or "Music" in title_str or "Музыка" in title_str:
                                return page.get("webSocketDebuggerUrl")
                        if pages: return pages[0].get("webSocketDebuggerUrl")
        except: return None

    async def ensure_connection(self):
         return self.is_connected

    @property
    def is_playing(self) -> bool: return self.last_state.playback.is_playing
    
    @property
    def playback_state(self) -> PlaybackData: return self.last_state.playback
    
    @property
    def track_info(self) -> TrackData: return self.last_state.track
        
    @property
    def is_liked(self) -> bool: return self.last_state.like.is_liked

    @property
    def is_disliked(self) -> bool: return self.last_state.dislike.is_disliked

    @property
    def volume(self) -> float: return self.last_state.volume.current

    @property
    def is_muted(self) -> bool: return self.last_state.volume.is_muted

    async def _exec_command(self, method: JSMethod, *args) -> ActionResultData:
        """
        Формирует и выполняет вызов js метода, автоматически форматируя аргументы.
        
        Returns:
            ActionResultData
        """
        if not await self._ensure_injection():
             return ActionResultData(success=False, error="Injection failed")
        formatted_args = []
        for arg in args:
            formatted_args.append(f"'{arg}'" if isinstance(arg, str) else str(arg))
        args_str = ", ".join(formatted_args)
        script = f"{JS_CONTROLLER_NAME}.{method}({args_str})"
        
        raw = await self.execute_script(script)
        if isinstance(raw, dict):
            return ActionResultData.from_dict(raw)
        return ActionResultData(success=False, error="Invalid response type")

    async def fetch_state(self) -> MediaState:
        """Получает полное текущее состояние плеера.
        
        Returns:
            MediaState
        """
        script = f"{JS_CONTROLLER_NAME}.{JSMethod.GET_FULL_STATE}()"
        raw = await self.execute_script(script)
        
        if raw and isinstance(raw, dict) and raw.get("success"):
            return MediaState.from_dict(raw.get("data"))
        return MediaState()

    async def play_pause(self):
        result = await self._exec_command(JSMethod.PLAY_PAUSE)
        if result.success and result.is_playing is not None:
             self.last_state.playback.is_playing = result.is_playing
             self._playback_optimistic_until = time.time() + 2.0
             self.last_state.playback.timestamp = time.time()
             self._notify_observers(EventType.PLAYBACK, self.last_state.playback)
        return result
    
    async def next_track(self):
        return await self._exec_command(JSMethod.NEXT)

    async def previous_track(self):
        return await self._exec_command(JSMethod.PREV)
    
    async def toggle_like(self):
        result = await self._exec_command(JSMethod.TOGGLE_LIKE)
        if result.success and result.new_state is not None:
             self.last_state.like.is_liked = result.new_state
             self._notify_observers(EventType.LIKE, self.last_state.like)
        return result

    async def toggle_dislike(self):
        result = await self._exec_command(JSMethod.TOGGLE_DISLIKE)
        if result.success and result.new_state is not None:
             self.last_state.dislike.is_disliked = result.new_state
             self._notify_observers(EventType.DISLIKE, self.last_state.dislike)
        return result

    async def change_volume(self, action: str, value: float = 0):
        if self.is_connected:
            await self._ensure_injection()
        result = await self._exec_command(JSMethod.CHANGE_VOLUME, action, value)
        if result.success:
            updated = False
            if result.volume is not None:
                self.last_state.volume.current = result.volume
                updated = True
            if result.is_muted is not None:
                self.last_state.volume.is_muted = result.is_muted
                updated = True
            if updated:
                self._volume_optimistic_until = time.time() + 2.0
                self._notify_observers(EventType.VOLUME, self.last_state.volume)
                
        return result

def get_cdp_controller():
    return CDPMediaController()
