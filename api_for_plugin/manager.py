import uuid
import asyncio
import logging
from utils.auth import AuthStorage
from ynison.player import YnisonPlayer
from yandex_api import YandexMusicAPI
from typing import Optional, Set, Dict


logger = logging.getLogger("SessionManager")


class YnisonSession:
    def __init__(self, token: str, on_update_callback):
        self.token = token
        self.on_update_callback = on_update_callback
        self.ynison: Optional[YnisonPlayer] = None
        self.api_client: Optional[YandexMusicAPI] = None
        self.liked_tracks: Set[str] = set()
        self.disliked_tracks: Set[str] = set()
        self.track_cache: Dict[str, dict] = {}
        self.is_connected = False
        self.running = False
        
    async def start(self):
        if self.running: return
        self.running = True
        try:
            self.api_client = YandexMusicAPI(self.token)
            await self.api_client.init()
            likes = await self.api_client.get_liked_tracks()
            self.liked_tracks = set(likes)
            
            try:
                dislikes = await self.api_client.get_disliked_tracks()
                if dislikes:
                    self.disliked_tracks = set(dislikes)
            except: pass
            
        except PermissionError as e:
             logger.error(f"[{self.token[:4]}..] Auth Error: {e}")
             self.running = False
             await self.api_client.close()
             raise e
             
        except Exception as e:
            logger.error(f"[{self.token[:4]}..] API Init failed (metadata might be partial): {e}")
            
        asyncio.create_task(self.run_loop())
        
    async def run_loop(self):
        while self.running:
            try:
                storage = AuthStorage(token=self.token, device_id=str(uuid.uuid4()))
                caps = {
                    "can_be_player": False,
                    "can_be_remote_controller": True,
                    "volume_granularity": 16
                }
                
                self.ynison = YnisonPlayer(storage, capabilities=caps, is_shadow=True)
                self.ynison.on_receive = self.handle_ynison_state
                self.ynison.on_close = self.handle_close
                
                logger.info(f"[{self.token[:4]}..] Connecting to Ynison...")
                await self.ynison.connect()
                self.is_connected = True
                
                if self.ynison.state:
                    await self.handle_ynison_state(self.ynison.state)
                
                while self.is_connected and self.running:
                    await asyncio.sleep(1)
                    
            except Exception as e:
                logger.error(f"[{self.token[:4]}..] Ynison connection error: {e}")
                self.is_connected = False
            
            if self.running:
                await asyncio.sleep(5)
            
    async def handle_ynison_state(self, state):
        try:
            state_dict = state.model_dump(by_alias=True)
            
            if self.on_update_callback:
                await self.on_update_callback(self.token, state_dict)
            
            asyncio.create_task(self.enrich_and_broadcast(state_dict))
            
        except Exception as e:
            logger.error(f"[{self.token[:4]}..] State handle error: {e}")

    async def enrich_and_broadcast(self, state_dict):
        """Обогащает стейт метаданными и транслирует его повторно, когда данные готовы."""
        try:
            player_state = state_dict.get("player_state", {})
            queue = player_state.get("player_queue", {})
            items = queue.get("playable_list", [])
            idx = queue.get("current_playable_index", 0)
            
            if not items or not (0 <= idx < len(items)):
                return
            
            await self.enrich_state_dict(state_dict)
            
            if self.on_update_callback:
                tid = "unknown"
                try:
                    tid = state_dict["player_state"]["player_queue"]["playable_list"][idx]["playable_id"]
                except: pass
                
                logger.info(f"Broadcasting enriched state for track {tid}...")
                await self.on_update_callback(self.token, state_dict)
            else:
                logger.warning("No on_update_callback set in session!")
        except Exception as e:
            logger.error(f"Background enrichment failed: {e}")

    async def handle_close(self, *args):
        self.is_connected = False

    async def enrich_state_dict(self, state_dict):
        """Добавляет в стейт is_liked, is_disliked, имена исполнителей и URI обложек."""
        try:
            player_state = state_dict.get("player_state", {})
            if not player_state: return
            
            queue = player_state.get("player_queue", {})
            items = queue.get("playable_list", [])
            idx = queue.get("current_playable_index", 0)
            
            if items and 0 <= idx < len(items):
                track = items[idx]
                tid = str(track.get("playable_id"))
                
                if tid:
                    track["is_liked"] = tid in self.liked_tracks
                    track["is_disliked"] = tid in self.disliked_tracks
                    
                    if tid not in self.track_cache and self.api_client:
                        try:
                            logger.info(f"Enriching metadata for track {tid}...")
                            full_track = await self.api_client.get_track(tid)
                            if full_track:
                                artists = full_track.get("artists", [])
                                artist_names = [a.get("name") for a in artists if a.get("name")]
                                artists_str = ", ".join(artist_names)
                                
                                cover_uri = full_track.get("coverUri") or full_track.get("cover_uri")
                                
                                self.track_cache[tid] = {
                                    "artists_enriched": artists_str,
                                    "cover_uri_enriched": cover_uri
                                }
                        except Exception as e:
                            logger.error(f"Failed to fetch metadata for {tid}: {e}")

                    if tid in self.track_cache:
                        cache = self.track_cache[tid]
                        if cache.get("artists_enriched"):
                            track["artists_enriched"] = cache["artists_enriched"]
                        if cache.get("cover_uri_enriched"):
                            track["cover_uri_enriched"] = cache["cover_uri_enriched"]
        except Exception as e:
            logger.error(f"Enrich state error: {e}")

    async def play_pause(self):
        if self.ynison: await self.ynison.toggle_play_pause()
        
    async def next(self):
        if self.ynison: await self.ynison.next()
        
    async def prev(self):
        if self.ynison: await self.ynison.prev()
        
    async def like(self):
        if not self.ynison or not self.ynison.current_track: return
        tid = self.ynison.current_track.playable_id
        if not tid or not self.api_client: return
        
        try:
            if tid in self.liked_tracks:
                await self.api_client.unlike_track(tid)
                self.liked_tracks.discard(tid)
            else:
                await self.api_client.like_track(tid)
                self.liked_tracks.add(tid)
            
            if self.ynison.state:
                await self.handle_ynison_state(self.ynison.state)
        except Exception as e:
            logger.error(f"Like failed: {e}")

    async def dislike(self):
        if not self.ynison or not self.ynison.current_track: return
        tid = self.ynison.current_track.playable_id
        if not tid or not self.api_client: return
        
        try:
            if tid in self.disliked_tracks:
                await self.api_client.undislike_track(tid)
                self.disliked_tracks.discard(tid)
            else:
                await self.api_client.dislike_track(tid)
                self.disliked_tracks.add(tid)
            
            if self.ynison.state:
                await self.handle_ynison_state(self.ynison.state)
        except Exception as e:
            logger.error(f"Dislike failed: {e}")

    async def close(self):
        self.running = False
        if self.api_client:
            await self.api_client.close()
        
        if self.ynison and hasattr(self.ynison, 'close'):
             try:
                 if asyncio.iscoroutinefunction(self.ynison.close):
                     await self.ynison.close()
                 else:
                     self.ynison.close()
             except: pass


class SessionManager:
    def __init__(self):
        self.sessions: Dict[str, YnisonSession] = {}
        self.on_global_update = None 
        
    async def get_session(self, token: str) -> YnisonSession:
        if not token:
            raise ValueError("Token required")
            
        if token not in self.sessions:
            logger.info(f"Creating new session for token {token[:5]}...")
            session = YnisonSession(token, self.on_session_update)
            try:
                await session.start()
                self.sessions[token] = session
            except Exception as e:
                 logger.error(f"Failed to start session for {token[:5]}: {e}")
                 raise e
            
        return self.sessions[token]

    async def on_session_update(self, token, state):
        if self.on_global_update:
            await self.on_global_update(token, state)

    async def shutdown(self):
        logger.info("Shutting down SessionManager...")
        for token, session in self.sessions.items():
            logger.info(f"Closing session for {token[:5]}...")
            await session.close()
        self.sessions.clear()

    async def shutdown(self):
        logger.info("Shutting down SessionManager...")
        for token, session in self.sessions.items():
            logger.info(f"Closing session for {token[:5]}...")
            await session.close()
        self.sessions.clear()

    async def close(self):
        self.running = False
        if self.api_client:
            await self.api_client.close()
        if self.ynison:
             if hasattr(self.ynison, 'close'):
                 await self.ynison.close()
             elif hasattr(self.ynison, 'stop'):
                 await self.ynison.stop()
        self.is_connected = False
