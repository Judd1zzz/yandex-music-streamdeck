import time
import asyncio
from src.core.logger import Logger
from src.core.registry import action_handler
from src.actions.base import YandexMusicBaseAction
from src.actions.mixins import PlaybackObservable
from src.core.renderers.progress import ProgressRenderer
from src.core.schemas.events import WillAppearModel, WillDisappearModel


@action_handler("com.judd1.yandex_music.action.progress")
class Progress(PlaybackObservable, YandexMusicBaseAction):
    """Кнопка с прогресс-баром прослушивания"""
    def __init__(self, action: str, context: str, settings: dict, plugin, **kwargs):
        self.renderer = ProgressRenderer()
        super().__init__(action, context, settings, plugin, **kwargs)

    async def progress_loop(self):
        while True:
            try:
                mode = self.settings.get("control_mode", "local")
                should_sleep = False
                
                if mode == "local":
                    if not self.cdp.is_playing: should_sleep = True
                else:
                    if not self.client.is_ready or self.client.is_paused: should_sleep = True
                
                if should_sleep:
                    await asyncio.sleep(1.0)
                    continue

                await asyncio.sleep(0.5)
                await self.render()
            except asyncio.CancelledError:
                break
            except Exception as e:
                Logger.error(f"Progress Loop Error: {e}")
                await asyncio.sleep(1)

    async def on_will_appear(self, obj: WillAppearModel):
        await super().on_will_appear(obj)
        self.start_task("progress", self.progress_loop())

    async def on_will_disappear(self, obj: WillDisappearModel):
        await super().on_will_disappear(obj)

    async def render(self):
        mode = self.settings.get("progress_mode", "stacked")
        progress = 0
        duration = 0
        
        control_mode = self.settings.get("control_mode", "local")
        if control_mode == "local":
             if self.cdp.is_connected:
                 pb = self.cdp.playback_state
                 
                 progress = pb.current_sec * 1000
                 duration = pb.total_sec * 1000
                 
                 if pb.is_playing:
                     elapsed = time.time() - pb.timestamp
                     progress += elapsed * 1000
                     if progress > duration: progress = duration
        
        if control_mode != "local" and self.client.current_state:
            ps = self.client.current_state.get("player_state", {})
            st = ps.get("status", {})
            progress = st.get("progress_ms", 0)
            duration = st.get("duration_ms", 0)

            if not self.client.is_paused and self.client.last_state_update_time > 0:
                elapsed_ms = (time.time() - self.client.last_state_update_time) * 1000
                progress += elapsed_ms
                if progress > duration: progress = duration
            
        loop = asyncio.get_running_loop()
        b64 = await loop.run_in_executor(
            None, 
            self.renderer.render, 
            progress, duration, 144, 144, mode
        )
        
        await self.set_image(b64, is_b64=True)
