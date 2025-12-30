from src.core.types import YnisonCommand
from src.core.registry import action_handler
from src.core.schemas.events import KeyDownModel
from src.actions.base import YandexMusicBaseAction

@action_handler("com.judd1.yandex_music.action.prev")
class PrevTrack(YandexMusicBaseAction):
    def should_ignore_errors(self):
        return self.settings.get("control_mode", "local") == "local"
    
    async def render_action(self):
        mode = self.settings.get("control_mode", "local")
        
        style = self.settings.get("prev_style", "v1")
        
        if mode == "local":
            if not self.cdp.is_connected:
                await self.set_image(f"btn_yandex_music_prev_{style}_loading.png")
                return

        elif not self.should_ignore_errors():
            if not self.client.is_ready:
                await self.set_image(f"btn_yandex_music_prev_{style}_loading.png")
                return
        
        await self.set_image(f"btn_yandex_music_prev_{style}.png")

    async def on_key_down(self, obj: KeyDownModel):
        mode = self.settings.get("control_mode", "local")
        if mode == "local":
            await self.cdp.previous_track()
        else:
            await self.client.send_command(YnisonCommand.PREV)
