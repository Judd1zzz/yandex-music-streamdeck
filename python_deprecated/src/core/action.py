import os
import base64
import asyncio
from src.core.logger import Logger
from typing import Any, Dict, Optional
from src.core.cache import get_image_cache, get_static_cache
from src.core.renderer import draw_button_image, fetch_image


image_cache = get_image_cache()
static_cache = get_static_cache()


class Action:
    def __init__(self, action: str, context: str, settings: Dict, plugin):
        self.action = action
        self.context = context
        self.settings = settings
        self.plugin = plugin
        self.title = ""
        
        self._last_render_state = None
        self._is_rendering = False
        
        self.needs_animation = False
        self.animation_task = None

    async def send_to_property_inspector(self, payload: Any):
        await self.plugin.send_json({
            'event': 'sendToPropertyInspector',
            'action': self.action,
            'context': self.context,
            'payload': payload
        })
    
    async def set_state(self, state: int):
        await self.plugin.send_json({
            'event': 'setState',
            'context': self.context,
            'payload': {'state': state}
        })
    
    async def set_title(self, title: str):
        await self.plugin.send_json({
            'event': 'setTitle',
            'context': self.context,
            'payload': {'title': title, 'target': 0}
        })
    
    async def set_settings(self, payload: Any):
        self.settings = payload
        await self.plugin.send_json({
            'event': 'setSettings',
            'context': self.context,
            'payload': payload
        })
    
    async def open_url(self, url: str):
        await self.plugin.send_json({
            'event': 'openUrl',
            'payload': {'url': url}
        })
    
    async def show_ok(self):
        await self.plugin.send_json({
            'event': 'showOk',
            'context': self.context
        })
    
    async def show_alert(self):
        await self.plugin.send_json({
            'event': 'showAlert',
            'context': self.context
        })

    async def set_image(self, filename_or_b64: str, is_b64: bool = False, state: Optional[int] = None):
        # if not filename_or_b64.startswith('data:image/'):
        #    Logger.info(f"request to set image: {filename_or_b64}")  # для отладки
        
        url = filename_or_b64 if is_b64 else await self._load_image_b64_async(filename_or_b64)
        if url:
             payload = {'target': 0, 'image': url}
             if state is not None:
                 payload['state'] = state

             await self.plugin.send_json({
                'event': 'setImage',
                'context': self.context,
                'payload': payload
             })
    
    async def log_message(self, message: str):
        await self.plugin.send_json({
            'event': 'logMessage',
            'payload': {'message': message}
        })

    async def render_optimized(self, track_data: Dict, icon_name: Optional[str] = None):
        if self._is_rendering:
            return
            
        self._is_rendering = True
        try:
            self._latest_track_data = track_data
            self._latest_icon_name = icon_name
            
            cover_url = track_data.get("cover_url", "")
            title = track_data.get("title", "")
            artist = track_data.get("artist", "")
            
            current_title_sig = f"{title}-{artist}"
            if not hasattr(self, '_last_title_sig') or self._last_title_sig != current_title_sig:
                self._last_title_sig = current_title_sig
                self._animation_offset = 0
            
            base_image = None
            if cover_url:
                base_image = image_cache.get(cover_url)
                if not base_image:
                    import aiohttp
                    async with aiohttp.ClientSession() as session:
                        raw_image = await fetch_image(session, cover_url)
                        if raw_image:
                             from PIL import Image
                             TARGET_SIZE = (72, 72)
                             if raw_image.size != TARGET_SIZE:
                                 base_image = raw_image.resize(TARGET_SIZE, Image.Resampling.BILINEAR)
                             else:
                                 base_image = raw_image
                             
                             image_cache.put(cover_url, base_image)

            icon_overlay = None
            if icon_name:
                path = os.path.join(os.getcwd(), "static/img", icon_name)
                if os.path.exists(path):
                     def load_icon():
                         from PIL import Image
                         return Image.open(path).convert("RGBA")
                     icon_overlay = await asyncio.to_thread(load_icon)

            loop = asyncio.get_running_loop()
            from functools import partial
            
            offset = getattr(self, '_animation_offset', 0)
            
            draw_func = partial(
                draw_button_image,
                base_image=base_image,
                icon_overlay=icon_overlay,
                title=title if self.settings.get('show_title', True) else "",
                artist=artist if self.settings.get('show_artist', True) else "",
                animation_offset=offset
            )
            
            b64_result, needs_animation = await loop.run_in_executor(None, draw_func)
            
            await self.set_image(b64_result, is_b64=True)
            
            if needs_animation:
                if not self.animation_task or self.animation_task.done():
                    self.animation_task = asyncio.create_task(self._animation_loop())
            else:
                if self.animation_task and not self.animation_task.done():
                    self.animation_task.cancel()
                    self.animation_task = None
            
        except Exception as e:
            Logger.error(f"Render Error: {e}")
        finally:
            self._is_rendering = False

    async def _animation_loop(self):
        try:
            while True:
                await asyncio.sleep(0.15) 
                self._animation_offset = getattr(self, '_animation_offset', 0) + 4 
                
                if hasattr(self, '_latest_track_data'):
                    await self.render_optimized(self._latest_track_data, getattr(self, '_latest_icon_name', None))
                else:
                    break
                
        except asyncio.CancelledError:
            pass
        except Exception as e:
            Logger.error(f"Anim Loop Error: {e}")

    async def _load_image_b64_async(self, filename: str):
        cached = static_cache.get(filename)
        if cached:
            return cached

        path = os.path.abspath(os.path.join(os.getcwd(), "static/img", filename))
        if os.path.exists(path):
            try:
                loop = asyncio.get_running_loop()
                def read():
                    with open(path, "rb") as f:
                        return base64.b64encode(f.read()).decode('utf-8')
                b64 = await loop.run_in_executor(None, read)
                result = f"data:image/png;base64,{b64}"
                
                static_cache.put(filename, result)
                # Logger.info(f"Added to cache file: {filename}")  # для отладки
                return result
            except Exception as e:
                Logger.error(f"Image load failed: {e}")
        else:
            Logger.error(f"Image not found: {filename}")
        return None
