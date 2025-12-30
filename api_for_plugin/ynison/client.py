import ssl
import json
import socket
import logging
import aiohttp
from aiohttp import TCPConnector
from utils.auth import AuthStorage
from typing import Optional, Callable, Awaitable


logger = logging.getLogger(__name__)


class YnisonWebSocket:
    def __init__(self, storage: AuthStorage):
        self.storage = storage
        self._ws: Optional[aiohttp.ClientWebSocketResponse] = None
        self._session: Optional[aiohttp.ClientSession] = None
        self._running = False
        self.on_receive: Optional[Callable[[str], Awaitable[None]]] = None
        self.on_close: Optional[Callable[[int, str], Awaitable[None]]] = None

    @property
    def is_connected(self) -> bool:
        return self._ws is not None and not self._ws.closed

    async def _get_protocol_data(self, device_id: str, redirect_ticket: Optional[str] = None, session_id: Optional[str] = None) -> str:
        device_info = {
            "app_name": "Desktop",
            "app_version": "5.79.7",
            "type": 1
        }
        
        device_info_str = json.dumps(device_info, separators=(',', ':'))
        
        protocol = {
            "Ynison-Device-Id": device_id,
        }
        
        if redirect_ticket:
            protocol["Ynison-Redirect-Ticket"] = redirect_ticket
            
        if session_id:
            protocol["Ynison-Session-Id"] = session_id

        protocol["Ynison-Device-Info"] = device_info_str
        
        protocol["authorization"] = f"OAuth {self.storage.token}"
        protocol["X-Yandex-Music-Multi-Auth-User-Id"] = self.storage.user_id
            
        return json.dumps(protocol, separators=(',', ':'))

    async def connect(self, url: str, redirect_ticket: Optional[str] = None, session_id: Optional[str] = None) -> bool:
        protocol_data = await self._get_protocol_data(self.storage.device_id, redirect_ticket, session_id)
        
        if redirect_ticket:
            logger.info(f"TICKET DETECTED! Header preview: Bearer, v2, {protocol_data[:50]}...")
        else:
            logger.info("No ticket in this connection.")

        if self._session is None:
            connector = TCPConnector(family=socket.AF_INET)
            self._session = aiohttp.ClientSession(connector=connector)

        headers = {
            "Origin": "https://music.yandex.ru",
            "Authorization": f"OAuth {self.storage.token}",
            "Sec-WebSocket-Protocol": f"Bearer, v2, {protocol_data}",
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        }

        ssl_context = ssl._create_unverified_context()
        ssl_context.check_hostname = False
        ssl_context.verify_mode = ssl.CERT_NONE

        try:
            self._ws = await self._session.ws_connect(
                url,
                headers=headers,
                autoping=True,
                heartbeat=30,
                protocols=("Bearer", "v2", protocol_data),
                ssl=ssl_context,
                timeout=20.0
            )
            self._running = True
            logger.info(f"✅ Connected to {url}")
            return True
        except Exception as e:
            logger.error(f"❌ Failed to connect to {url}: {e}")
            return False

    async def begin_receive(self):
        if not self._ws:
            return

        try:
            async for msg in self._ws:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    if self.on_receive:
                        await self.on_receive(msg.data)
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    logger.error('ws connection closed with exception %s', self._ws.exception())
        finally:
            self._running = False
            if self.on_close:
                close_message = getattr(self._ws, 'close_message', "")
                await self.on_close(self._ws.close_code or 1000, close_message)

    async def send(self, data: str):
        if self._ws:
            await self._ws.send_str(data)

    async def close(self):
        self._running = False
        if self._ws:
            await self._ws.close()
        if self._session:
            await self._session.close()

    async def stop_receive(self):
         await self.close()
