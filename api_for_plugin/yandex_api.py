import logging
import aiohttp
from typing import List, Optional


logger = logging.getLogger("YandexMusicAPI")


class YandexMusicAPI:
    BASE_URL = "https://api.music.yandex.net"
    HEADERS = {
        "X-Yandex-Music-Client": "YandexMusicAndroid/24023621",
        "User-Agent": "Yandex-Music-API",
    }

    def __init__(self, token: str):
        self.token = token
        self.uid: Optional[str] = None
        self._session: Optional[aiohttp.ClientSession] = None

    async def init(self):
        """Инициализирует сессию клиента и получает ID пользователя."""
        timeout = aiohttp.ClientTimeout(total=10)
        self._session = aiohttp.ClientSession(
            headers={
                **self.HEADERS,
                "Authorization": f"OAuth {self.token}"
            },
            timeout=timeout
        )
        await self._fetch_uid()

    async def close(self):
        if self._session:
            await self._session.close()

    async def _fetch_uid(self):
        """Получает ID пользователя из статуса аккаунта."""
        try:
            async with self._session.get(f"{self.BASE_URL}/account/status") as resp:
                logger.info(f"Account Status Check: {resp.status}")
                if resp.status == 200:
                    data = await self._safe_json(resp)
                    self.uid = str(data.get("account", {}).get("uid"))
                else:
                    logger.error(f"Failed to fetch account status: {resp.status}")
                    if resp.status == 401:
                        raise PermissionError("Invalid Token")
        except PermissionError:
            raise
        except Exception as e:
            logger.error(f"Error fetching UID: {e}")

    async def _safe_json(self, resp):
        """Обрабатывает ответы как обёрнутые в {"result": ...}, так и прямые."""
        try:
            data = await resp.json()
            if isinstance(data, dict) and "result" in data:
                return data["result"]
            return data
        except Exception as e:
            logger.error(f"JSON Parse Error: {e}")
            return {}

    async def get_liked_tracks(self) -> List[str]:
        if not self.uid: return []
        try:
            url = f"{self.BASE_URL}/users/{self.uid}/likes/tracks"
            async with self._session.get(url) as resp:
                if resp.status == 200:
                    data = await self._safe_json(resp)
                    tracks = data.get("library", {}).get("tracks", []) if isinstance(data, dict) else []
                    return [str(track.get("id")) for track in tracks if track.get("id")]
                return []
        except Exception as e:
            logger.error(f"Error fetching liked tracks: {e}")
            return []

    async def get_disliked_tracks(self) -> List[str]:
        if not self.uid: return []
        try:
            url = f"{self.BASE_URL}/users/{self.uid}/dislikes/tracks"
            async with self._session.get(url) as resp:
                if resp.status == 200:
                    data = await self._safe_json(resp)
                    tracks = data.get("library", {}).get("tracks", []) if isinstance(data, dict) else []
                    return [str(track.get("id")) for track in tracks if track.get("id")]
                return []
        except Exception as e:
            logger.error(f"Error fetching disliked tracks: {e}")
            return []

    async def _like_action(self, track_id: str, action: str, type_: str = "likes") -> bool:
        if not self.uid: return False
        try:
            url = f"{self.BASE_URL}/users/{self.uid}/{type_}/tracks/{action}"
            data = {f"track-ids": str(track_id)}
            async with self._session.post(url, data=data) as resp:
                logger.info(f"Action {type_}/{action} Status: {resp.status}")
                return resp.status == 200
        except Exception as e:
            logger.error(f"Error performing {type_}/{action}: {e}")
            return False

    async def like_track(self, track_id: str) -> bool:
        return await self._like_action(track_id, "add-multiple", "likes")

    async def unlike_track(self, track_id: str) -> bool:
        return await self._like_action(track_id, "remove", "likes")

    async def dislike_track(self, track_id: str) -> bool:
        return await self._like_action(track_id, "add-multiple", "dislikes")

    async def undislike_track(self, track_id: str) -> bool:
        return await self._like_action(track_id, "remove", "dislikes")

    async def get_track(self, track_id: str) -> Optional[dict]:
        """Получает подробную информацию об одном треке."""
        tracks = await self.get_tracks([track_id])
        return tracks[0] if tracks else None

    async def get_tracks(self, track_ids: List[str]) -> List[dict]:
        """Получает подробную информацию о нескольких треках (POST-запрос, как в оригинальной библиотеке)."""
        if not track_ids: return []
        try:
            url = f"{self.BASE_URL}/tracks"
            ids_str = ",".join(map(str, track_ids))
            data = {"track-ids": ids_str}
            
            async with self._session.post(url, data=data) as resp:
                logger.info(f"Get Tracks API Status: {resp.status} for {len(track_ids)} IDs")
                if resp.status == 200:
                    result = await self._safe_json(resp)
                    if isinstance(result, list):
                         logger.info(f"Successfully fetched {len(result)} tracks info")
                         return result
                    else:
                         logger.warning(f"Get Tracks returned unexpected type: {type(result)}")
                         return []
                else:
                    err_text = await resp.text()
                    logger.error(f"Get Tracks failed with status {resp.status}: {err_text[:200]}")
                return []
        except Exception as e:
            logger.error(f"Error fetching tracks: {e}")
            return []
