import json
import time
import uuid
import logging
import asyncio
from utils.auth import AuthStorage
from ynison.client import YnisonWebSocket
from ynison.models.common import YnisonVersion
from typing import Optional, Callable, Awaitable
from ynison.models.redirect import YnisonRedirect
from ynison.models.state import YnisonState
from ynison.models.messages import YnisonFullState
from ynison.models.player_state import YnisonPlayerState
from ynison.models.device import YnisonDeviceFull, YnisonDevice


logger = logging.getLogger(__name__)


class YnisonPlayer:
    def __init__(self, storage: AuthStorage, device_info: Optional[dict] = None, 
                 capabilities: Optional[dict] = None, is_shadow: bool = True):
        self.storage = storage
        self.redirector = YnisonWebSocket(storage)
        self.is_shadow = is_shadow
        self.device_info = device_info or {
            "app_name": "Yandex Music API",
            "app_version": "0.0.1",
            "type": "WEB",
            "title": "Deck Player"
        }
        self.capabilities = capabilities or {
            "can_be_player": True,
            "can_be_remote_controller": True,
            "volume_granularity": 16
        }
        self.state_socket = YnisonWebSocket(storage)
        self.state: Optional[YnisonState] = None
        self._last_state_data: Optional[dict] = None
        self._current_track = None
        self._last_update_time = 0
        

        
        self.on_receive: Optional[Callable[[YnisonState], Awaitable[None]]] = None
        self.on_close: Optional[Callable[[int, str], Awaitable[None]]] = None
        
        self.state_socket.on_receive = self._process_ws_message
        self.state_socket.on_close = self._handle_close

    def _default_state(self) -> str:
        version = YnisonVersion(device_id=self.storage.device_id, version="0")
        
        full_state = YnisonUpdateFullStateMessage(
            update_full_state=YnisonFullState(
                device=YnisonDevice(
                    capabilities=self.capabilities,
                    info={
                        "device_id": self.storage.device_id,
                        "app_name": self.device_info["app_name"],
                        "app_version": self.device_info["app_version"],
                        "type": self.device_info["type"],
                        "title": self.device_info["title"]
                    },
                    is_shadow=self.is_shadow,
                    volume_info={"volume": 0}
                ),
                player_state=YnisonPlayerState(
                    player_queue={"version": version},
                    status={"version": version, "duration_ms": 0, "progress_ms": 0}
                ),
                is_currently_active=False
            )
        )
        return full_state.model_dump_json(exclude_none=True, by_alias=True)

    async def connect(self):
        REDIRECT_URL = "wss://ynison.music.yandex.ru/redirector.YnisonRedirectService/GetRedirectToYnison"
        
        logger.info(f"Connecting to Redirector: {REDIRECT_URL}")
        
        if not await self.redirector.connect(REDIRECT_URL):
            raise Exception("Failed to connect to Redirector service")

        try:
            response_data = await self.redirector._ws.receive_str()
        except Exception as e:
            raise Exception(f"Failed to receive response from Redirector: {e}")

        try:
            json_data = json.loads(response_data)
            if 'error' in json_data:
                error_info = json_data['error']
                error_msg = f"Ynison Server Error: {error_info.get('message', 'Unknown')} (Code: {error_info.get('grpc_code')})"
                logger.error(error_msg)
                raise Exception(error_msg)
        except json.JSONDecodeError:
            pass

        try:
            redirect = YnisonRedirect.model_validate_json(response_data)
            logger.info(f"Redirect received. Host: {redirect.host}")
        except Exception as e:
            logger.error(f"Validation failed. Raw response: {response_data}")
            raise e

        logger.info(f"Redirect received. Host: {redirect.host}")

        clean_host = redirect.host.replace("wss://", "").replace("https://", "").strip("/")
        state_url = f"wss://{clean_host}/ynison_state.YnisonStateService/PutYnisonState"

        logger.info(f"Connecting to State Socket: {state_url}")


        if await self.state_socket.connect(state_url, redirect_ticket=redirect.redirect_ticket, session_id=redirect.session_id):
            logger.info("✅ State Socket Connected! Starting receiver...")
            asyncio.create_task(self.state_socket.begin_receive())
            
            logger.info("Sending initial state...")
            payload = self._default_state()
            logger.info(f"Initial State Payload: {payload}")
            await self.state_socket.send(payload)
        else:
            raise Exception("Failed to connect to State Socket")



    async def _handle_close(self, code: int, reason: str):
        if self.on_close:
            await self.on_close(code, reason)

    async def close(self):
        await self.state_socket.stop_receive()
        await self.redirector.stop_receive()

    async def _send_one_off_command(self, payload: dict):
        """
        Отправляет команду через временное подключение со случайным Device ID, для избежания ошибок 1006.
        """
        temp_device_id = str(uuid.uuid4())
        temp_storage = AuthStorage(token=self.storage.token, device_id=temp_device_id)
        
        temp_redirector = YnisonWebSocket(temp_storage)
        temp_state_socket = YnisonWebSocket(temp_storage)
        
        try:
            logger.info(f"Initiating One-Off Command Connection (Device ID: {temp_device_id})")
            
            REDIRECT_URL = "wss://ynison.music.yandex.ru/redirector.YnisonRedirectService/GetRedirectToYnison"
            if not await temp_redirector.connect(REDIRECT_URL):
                logger.error("One-Off: Failed to connect to redirector")
                return

            response_data = await temp_redirector._ws.receive_str()
            redirect = YnisonRedirect.model_validate_json(response_data)
            
            clean_host = redirect.host.replace("wss://", "").replace("https://", "").strip("/")
            state_url = f"wss://{clean_host}/ynison_state.YnisonStateService/PutYnisonState"
            
            if await temp_state_socket.connect(state_url, redirect_ticket=redirect.redirect_ticket, session_id=redirect.session_id):
                 logger.info("One-Off: Connected to State Socket. Sending payload...")
                 await temp_state_socket.send(json.dumps(payload))
                 
                 await asyncio.sleep(0.5) 
                 logger.info("One-Off: Payload sent.")
            else:
                 logger.error("One-Off: Failed to connect to state socket")

        except Exception as e:
            logger.error(f"One-Off Command Failed: {e}")
        finally:
            await temp_state_socket.close()
            await temp_redirector.close()

    def _update_current_track(self):
        """
        Обновляет атрибут _current_track на основе текущего состояния.
        """
        if not self.state or not self.state.player_state:
            self._current_track = None
            return

        queue = self.state.player_state.player_queue
        idx = queue.current_playable_index

        if idx < 0 or idx >= len(queue.playable_list):
            self._current_track = None
            return

        self._current_track = queue.playable_list[idx]

    async def _process_ws_message(self, message: str):
        try:
            data = json.loads(message)
            self._last_update_time = time.time()

            if 'update_full_state' in data:
                try:
                    full_msg = YnisonUpdateFullStateMessage(**data)
                    full = full_msg.update_full_state
                    
                    new_player_state = full.player_state
                    
                    device_data = full.device.model_dump()
                    if 'volume' not in device_data:
                        vol = full.device.volume_info.volume if full.device.volume_info else 0.0
                        device_data['volume'] = vol
                    
                    new_device = YnisonDeviceFull(**device_data)

                    if self.state is None:
                        self.state = YnisonState(
                            rid=full_msg.rid,
                            devices=[new_device],
                            player_state=new_player_state,
                            timestamp_ms=full_msg.player_action_timestamp_ms or (time.time() * 1000)
                        )
                        logger.info(f"Initialized YnisonState from Full State (Device: {new_device.info.title})")
                    else:
                        self.state.player_state = new_player_state
                        found = False
                        for i, dev in enumerate(self.state.devices):
                            if dev.info.device_id == new_device.info.device_id:
                                self.state.devices[i] = new_device
                                found = True
                                break
                        if not found:
                            self.state.devices.append(new_device)
                        
                        if full_msg.player_action_timestamp_ms:
                            self.state.timestamp_ms = full_msg.player_action_timestamp_ms
                            
                    self._update_current_track()
                    
                    if self.on_receive:
                        await self.on_receive(self.state)

                except Exception as e:
                    logger.error(f"Failed to process update_full_state: {e}", exc_info=True)

            elif 'player_state' in data:
                 try:
                     try:
                         self.state = YnisonState(**data)
                     except:
                         if self.state:
                             ps = YnisonPlayerState(**data['player_state'])
                             self.state.player_state = ps
                         else:
                             pass
                     
                     if self.state:
                         self._update_current_track()
                         if self.on_receive:
                             await self.on_receive(self.state)
                             
                 except Exception as e:
                     logger.error(f"Failed to parse YnisonState/PlayerState: {e}")

        except json.JSONDecodeError:
            logger.error("Failed to decode JSON message")

    @property
    def current_track(self):
        """
        Возвращает текущий проигрываемый трек из состояния, или None если ничего не играет.
        """
        return self._current_track


    async def toggle_play_pause(self):
        if not self.state or not self.state.player_state:
            return
        
        st = self.state.player_state.status
        is_pausing = not st.paused
        
        new_progress = self.calculate_current_progress() if is_pausing else st.progress_ms
        
        
        pq = self.state.player_state.player_queue
        
        current_ts = time.time_ns()
        temp_device_id = str(uuid.uuid4())
        
        def build_version():
            return {
                "device_id": temp_device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }

        version_obj = build_version()

        status_payload = {
            "duration_ms": st.duration_ms,
            "progress_ms": new_progress,
            "paused": True if is_pausing else False,
            "playback_speed": st.playback_speed,
            "version": version_obj
        }
        
        queue_payload = {
             "entity_id": pq.entity_id,
             "entity_type": pq.entity_type,
             "current_playable_index": pq.current_playable_index,
             "playable_list": [
                {
                    "album_id_optional": item.album_id_optional,
                    "from": item.from_,
                    "playable_id": item.playable_id,
                    "playable_type": item.playable_type,
                    "title": item.title,
                    "cover_url_optional": item.cover_url_optional,
                    "navigation_id_optional": item.navigation_id_optional, 
                    "playback_action_id_optional": item.playback_action_id_optional
                } for item in pq.playable_list
             ],
             "shuffle_optional": None, 
             "options": pq.options.dict(),
             "entity_context": pq.entity_context,
             "from_optional": pq.from_optional,
             "initial_entity_optional": None,
             "adding_options_optional": None,
             "queue": pq.queue.dict() if pq.queue else None,
             "version": version_obj
        }

        payload_dict = {
            "update_player_state": {
                "player_state": {
                    "player_queue": queue_payload,
                    "status": status_payload
                }
            },
            "player_action_timestamp_ms": current_ts,
            "activity_interception_type": "DO_NOT_INTERCEPT_BY_DEFAULT" 
        }
            
        await self._send_one_off_command(payload_dict)

    async def play_track(self, track_id: str):
        """
        Запускает воспроизведение трека по ID.
        """
        current_ts = time.time_ns()
        temp_device_id = str(uuid.uuid4())
        
        queue_payload = {
            "current_playable_index": 0,
            "entity_id": "",
            "entity_type": "VARIOUS",
            "playable_list": [{"playable_id": str(track_id), "playable_type": "TRACK"}],
            "options": {"repeat_mode": "NONE"},
            "entity_context": "BASED_ON_ENTITY_BY_DEFAULT",
            "version": {
                "device_id": temp_device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }
        }

        status_payload = {
            "duration_ms": 0,
            "paused": False,
            "playback_speed": 1,
            "progress_ms": 0,
            "version": {
                "device_id": temp_device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }
        }

        payload_dict = {
            "update_player_state": {
                "player_state": {
                    "player_queue": queue_payload,
                    "status": status_payload
                }
            }
        }

        await self._send_one_off_command(payload_dict)

    def calculate_current_progress(self) -> int:
        if not self.state or not self.state.player_state:
            return 0
        st = self.state.player_state.status
        
        if st.paused:
             return st.progress_ms
        
        current_time = time.time()
        delta_sec = current_time - self._last_update_time
        delta_ms = delta_sec * 1000 * st.playback_speed
        
        val = int(st.progress_ms + delta_ms)
        return max(0, val)

    async def next(self):
        if not self.state or not self.state.player_state:
            return
        pq = self.state.player_state.player_queue
        
        new_index = pq.current_playable_index + 1
        
        if new_index >= len(pq.playable_list):
            logger.warning("Next track index out of bounds")
            
        current_ts = time.time_ns()
        temp_device_id = str(uuid.uuid4())
        
        def build_version():
            return {
                "device_id": temp_device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }

        status_payload = {
            "duration_ms": 0,
            "paused": False,
            "playback_speed": 1,
            "progress_ms": 0,
            "version": build_version()
        }
        
        queue_payload = {
             "entity_id": pq.entity_id,
             "entity_type": pq.entity_type,
             "current_playable_index": new_index,
             "playable_list": [
                {
                    "album_id_optional": item.album_id_optional,
                    "from": item.from_,
                    "playable_id": item.playable_id,
                    "playable_type": item.playable_type,
                    "title": item.title,
                    "cover_url_optional": item.cover_url_optional,
                    "navigation_id_optional": item.navigation_id_optional,
                    "playback_action_id_optional": item.playback_action_id_optional
                } for item in pq.playable_list
             ],
             "options": pq.options.dict(),
             "entity_context": pq.entity_context,
             "version": build_version()
        }

        payload_dict = {
            "update_player_state": {
                "player_state": {
                    "player_queue": queue_payload,
                    "status": status_payload
                }
            }
        }
        
        await self._send_one_off_command(payload_dict)

    async def prev(self):
        if not self.state or not self.state.player_state:
            return
        pq = self.state.player_state.player_queue
        
        new_index = pq.current_playable_index - 1
        if new_index < 0:
            new_index = 0
            
        current_ts = time.time_ns()
        temp_device_id = str(uuid.uuid4())
        
        def build_version():
            return {
                "device_id": temp_device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }

        status_payload = {
            "duration_ms": 0,
            "paused": False,
            "playback_speed": 1,
            "progress_ms": 0,
            "version": build_version()
        }
        
        queue_payload = {
             "entity_id": pq.entity_id,
             "entity_type": pq.entity_type,
             "current_playable_index": new_index,
             "playable_list": [
                {
                    "album_id_optional": item.album_id_optional,
                    "from": item.from_,
                    "playable_id": item.playable_id,
                    "playable_type": item.playable_type,
                    "title": item.title,
                    "cover_url_optional": item.cover_url_optional,
                    "navigation_id_optional": item.navigation_id_optional,
                    "playback_action_id_optional": item.playback_action_id_optional
                } for item in pq.playable_list
             ],
             "options": pq.options.dict(),
             "entity_context": pq.entity_context,
             "version": build_version()
        }

        payload_dict = {
            "update_player_state": {
                "player_state": {
                    "player_queue": queue_payload,
                    "status": status_payload
                }
            }
        }
        
        await self._send_one_off_command(payload_dict)

    async def update_state(self):
        if not self.state or not self.state.player_state:
            logger.warning("Cannot update state: No state available.")
            return

        pq = self.state.player_state.player_queue
        st = self.state.player_state.status
        current_ts = time.time_ns()
        
        def build_version():
            return {
                "device_id": self.storage.device_id,
                "version": current_ts,
                "timestamp_ms": 0
            }
        
        status_payload = {
            "duration_ms": st.duration_ms,
            "progress_ms": st.progress_ms,
            "paused": st.paused,
            "playback_speed": st.playback_speed,
            "version": build_version()
        }

        queue_payload = {
             "entity_id": pq.entity_id,
             "entity_type": pq.entity_type,
             "current_playable_index": pq.current_playable_index,
             "playable_list": [
                {
                    "album_id_optional": item.album_id_optional,
                    "from": item.from_,
                    "playable_id": item.playable_id,
                    "playable_type": item.playable_type,
                    "title": item.title,
                    "cover_url_optional": item.cover_url_optional,
                    "navigation_id_optional": None,
                    "playback_action_id_optional": None
                } for item in pq.playable_list
             ],
             "shuffle_optional": None,
             "options": pq.options.dict(),
             "entity_context": pq.entity_context,
             "from_optional": pq.from_optional,
             "initial_entity_optional": None,
             "adding_options_optional": None,
             "queue": pq.queue.dict() if pq.queue else None,
             "version": build_version()
        }

        payload_dict = {
            "update_player_state": {
                "player_state": {
                    "status": status_payload,
                    "player_queue": queue_payload
                }
            },
            "rid": str(uuid.uuid4()),
            "player_action_timestamp_ms": current_ts,
            "activity_interception_type": "DO_NOT_INTERCEPT_BY_DEFAULT"
        }

        logger.info(f"Update State Payload (Full State + Tweaks): {json.dumps(payload_dict)}")
        await self.state_socket.send(json.dumps(payload_dict))
