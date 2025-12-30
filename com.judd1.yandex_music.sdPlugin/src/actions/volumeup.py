import asyncio
from src.core.logger import Logger
from src.core.types import YnisonCommand
from src.core.registry import action_handler
from src.actions.base import YandexMusicBaseAction
from src.core.schemas.events import KeyDownModel, KeyUpModel, WillDisappearModel


@action_handler("com.judd1.yandex_music.action.volumeup")
class VolumeUp(YandexMusicBaseAction):
    """Кнопка увеличения громкости на 5%"""
    
    def __init__(self, action: str, context: str, settings: dict, plugin, **kwargs):
        super().__init__(action, context, settings, plugin, **kwargs)
        self._is_pressed = False

    async def render_action(self):
        style = self.settings.get("volume_style", "v1")
        mode = self.settings.get("control_mode", "local")
        await self.set_image(f"btn_yandex_music_vol_up_{style}.png")

        if mode == "local":
            if not self.cdp.is_connected:
                await self.set_image(f"btn_yandex_music_vol_up_{style}_loading.png")
                return

    async def _change_volume(self):
        mode = self.settings.get("control_mode", "local")
        if mode == "local":
            await self.cdp.change_volume("UP")
        else:
            await self.client.send_command(YnisonCommand.VOLUME_UP)

    async def _repeat_loop(self):
        try:
            for _ in range(5):
                if not self._is_pressed: return
                await asyncio.sleep(0.1)
                
            Logger.info("Volume Repeat Loop: Active")
            while self._is_pressed:
                await self._change_volume()
                await asyncio.sleep(0.1)
        except asyncio.CancelledError:
            pass
        except Exception as e:
            Logger.error(f"Volume Loop Exception: {e}")

    async def on_key_down(self, obj: KeyDownModel):
        self._is_pressed = True
        await self._change_volume()
        self.start_task("volume_repeat", self._repeat_loop())

    async def on_key_up(self, obj: KeyUpModel):
        self._is_pressed = False
        self.cancel_task("volume_repeat")

    async def on_will_disappear(self, obj: WillDisappearModel):
        self._is_pressed = False
        self.cancel_task("volume_repeat")
        await super().on_will_disappear(obj)
