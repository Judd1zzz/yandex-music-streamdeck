import asyncio
from io import BytesIO
from PIL import Image
from src.core.logger import Logger
from src.core.schemas.events import *
from src.core.registry import action_handler
from src.core.renderers.info import InfoRenderer
from src.actions.base import YandexMusicTrackAction


@action_handler("com.judd1.yandex_music.action.info")
class Info(YandexMusicTrackAction):
    def __init__(self, action: str, context: str, settings: dict, plugin, **kwargs):
        self.marquee_offset = 0
        self.needs_scrolling = False
        self.renderer = InfoRenderer()
        super().__init__(action, context, settings, plugin, **kwargs)

    async def animate_loop(self):
        while True:
            try:
                mode = self.cfg.control_mode
                is_paused = True
                
                if mode == "local":
                     if self.cdp.is_connected:
                         is_paused = not self.cdp.is_playing
                else:
                     is_paused = self.client.is_paused
                
                if is_paused:
                    await asyncio.sleep(0.5)
                    continue
                    
                if not self.needs_scrolling:
                    await asyncio.sleep(1.0)
                    continue
    
                await asyncio.sleep(1 / 5)
                self.marquee_offset += 5
                await self.render()

            except asyncio.CancelledError:
                break
            except Exception as e:
                Logger.error(f"Info Animation Error: {e}")
                await asyncio.sleep(1)

    async def render(self):
        mode = self.cfg.control_mode
        
        show_cover = self.cfg.show_cover
        show_title = self.cfg.show_title
        show_artist = self.cfg.show_artist
        
        cover_data = None
        cached_img_obj = None
        title = "Loading..."
        artists = ""
        
        if mode == "local":
             if self.cdp.is_connected:
                 info = self.cdp.track_info
                 title = info.title
                 artists = info.artist
                 cover_url = info.cover_url
                 
                 await self._update_local_cover(cover_url)
                 cached_img_obj = getattr(self, "local_cover_img", None)
                 cover_data = getattr(self, "local_cover_data", None)
             else:
                 title = "Waiting..."
                 
        else:
            cover_data = self.client.current_cover_data
            cached_img_obj = self.client.current_cover_img
            
            if self.client.current_track_data:
                title = self.client.current_track_data.get("title", "Unknown")
                artists = self.client.current_track_data.get("artists", "")
                if isinstance(artists, list):
                    names = []
                    for a in artists:
                        if isinstance(a, dict) and "name" in a: names.append(a["name"])
                        elif isinstance(a, str): names.append(a)
                    artists = ", ".join(names)
        
        loop = asyncio.get_running_loop()
        def draw():
             return self.renderer.render(
                cover_data=cover_data, 
                title=title, 
                artists=artists, 
                marquee_offset=self.marquee_offset,
                cached_cover_img=cached_img_obj,
                show_cover=show_cover,
                show_title=show_title,
                show_artists=show_artist
            )
        
        b64, needs_scroll = await loop.run_in_executor(None, draw)
        
        self.needs_scrolling = needs_scroll
        await self.set_image(b64, is_b64=True)

    async def _update_local_cover(self, url):
        if not url: return
        if getattr(self, "last_local_url", None) == url: return
            
        self.last_local_url = url
        try:
             import aiohttp
             async with aiohttp.ClientSession() as session:
                 if url.startswith("//"): url = "https:" + url
                 async with session.get(url) as resp:
                     if resp.status == 200:
                         data = await resp.read()
                         self.local_cover_data = data
                         loop = asyncio.get_running_loop()
                         def process():
                             raw = Image.open(BytesIO(data)).convert("RGBA")
                             return raw.resize((144, 144), Image.LANCZOS)
                         
                         self.local_cover_img = await loop.run_in_executor(None, process)
        except Exception as e:
            Logger.error(f"Local cover fetch failed: {e}")

    async def on_will_appear(self, obj: WillAppearModel):
        await super().on_will_appear(obj)
        self.start_task("marquee", self.animate_loop())

    async def on_will_disappear(self, obj: WillDisappearModel):
        await super().on_will_disappear(obj)

    async def on_did_receive_settings(self, obj: DidReceiveSettingsModel):
        await super().on_did_receive_settings(obj)

    async def on_key_down(self, obj: KeyDownModel):
        import sys
        import subprocess
        
        title = "Unknown"
        artist = "Unknown"
        
        mode = self.cfg.control_mode
        if mode == "local":
             if self.cdp.is_connected:
                 title = self.cdp.track_info.title
                 artist = self.cdp.track_info.artist
        else:
             if self.client.current_track_data:
                 title = self.client.current_track_data.get("title", "")
                 artists = self.client.current_track_data.get("artists", [])
                 if isinstance(artists, list):
                     names = []
                     for a in artists:
                         if isinstance(a, dict) and "name" in a: names.append(a["name"])
                         elif isinstance(a, str): names.append(a)
                     artist = ", ".join(names)

        if not title or title == "Unknown":
            return

        text_to_copy = f"{artist} - {title}"
        
        try:
            platform = sys.platform
            if platform == "darwin":
                process = subprocess.Popen('pbcopy', env={'LANG': 'en_US.UTF-8'}, stdin=subprocess.PIPE)
                process.communicate(input=text_to_copy.encode('utf-8'))
                Logger.info(f"Copied to clipboard (Mac): {text_to_copy}")
                
            elif platform == "win32":
                try:
                    subprocess.run('clip', input=text_to_copy.encode('utf-16le'), check=True, shell=True)
                except Exception:
                     subprocess.run('clip', input=text_to_copy.encode('cp1251', errors='replace'), check=True, shell=True)
                
                Logger.info(f"Copied to clipboard (Win): {text_to_copy}")
            await self.show_ok()
            
        except Exception as e:
            Logger.error(f"Clipboard Error: {e}")
            await self.show_alert()
