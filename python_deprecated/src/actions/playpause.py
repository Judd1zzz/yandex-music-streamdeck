from src.core.types import YnisonCommand
from src.core.registry import action_handler
from src.core.schemas.events import KeyDownModel
from src.actions.mixins import PlaybackObservable
from src.actions.base import YandexMusicBaseAction


@action_handler("com.judd1.yandex_music.action.playpause")
class PlayPause(PlaybackObservable, YandexMusicBaseAction):
    def should_ignore_errors(self):
        return self.cfg.control_mode == "local"

    async def on_playback_update(self, data):
        """Перерисовывает при изменении стейта (хэндлер апдейта с CDP)"""
        is_playing = data.is_playing
        if not hasattr(self, "_last_is_playing"):
             self._last_is_playing = None
        
        if self._last_is_playing != is_playing:
            self._last_is_playing = is_playing
            await self.render()
    async def render_action(self):
        mode = self.cfg.control_mode
        style = self.cfg.play_style
        is_paused = True
        
        if mode == "local":
             if not self.cdp.is_connected:
                  await self.set_image(f"btn_yandex_music_play_{style}_loading.png")
                  return
             is_paused = not self.cdp.is_playing
        else:
             if not self.should_ignore_errors():
                  if not self.client.is_ready:
                     await self.set_image(f"btn_yandex_music_play_{style}_loading.png")
                     return
             
             if self.client.current_state:
                  st = self.client.current_state.get("player_state", {}).get("status", {})
                  is_paused = st.get("paused", True)

        
        if is_paused:
            state = 1
            image = f"btn_yandex_music_play_{style}.png"
        else:
            state = 0
            pause_style = "v1" if style in ["v1", "v2"] else "v2"
            image = f"btn_yandex_music_pause_{pause_style}.png"
            
        await self.set_state(state)
        await self.set_image(image)

    async def on_key_down(self, obj: KeyDownModel):
        mode = self.cfg.control_mode
        if mode == "local":
            await self.cdp.play_pause()
        else:
            await self.client.send_command(YnisonCommand.PLAY_PAUSE)
