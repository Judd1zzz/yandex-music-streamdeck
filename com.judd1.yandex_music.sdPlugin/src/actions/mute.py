from src.core.registry import action_handler
from src.actions.mixins import VolumeObservable
from src.core.schemas.events import KeyDownModel
from src.actions.base import YandexMusicBaseAction


@action_handler("com.judd1.yandex_music.action.mute")
class Mute(VolumeObservable, YandexMusicBaseAction):
    async def render_action(self):
        is_muted = False
        mode = self.get_mode()
        style = self.settings.get("mute_style", "v1")
        
        if mode == "local":
            if self.cdp.is_connected:
                is_muted = self.cdp.is_muted
            else:
                await self.set_image(f"btn_yandex_music_mute_{style}_{'on' if is_muted else 'off'}_loading.png")
                self._last_image_name = "loading"
                return
        else:
            if self.client.current_state:
                for dev in self.client.current_state.get("devices", []):
                    if dev.get("info", {}).get("title") == "Deck Player": continue
                    if "volume_info" in dev: 
                        is_muted = dev["volume_info"].get("is_muted", False)

        suffix = "on" if is_muted else "off"
        image_name = f"btn_yandex_music_mute_{style}_{suffix}.png"
        
        if getattr(self, "_last_image_name", None) != image_name:
            # Logger.debug(f"[Mute] Setting image: {image_name}")
            await self.set_image(image_name)
            self._last_image_name = image_name

    async def on_key_down(self, obj: KeyDownModel):
        mode = self.settings.get("control_mode", "local")
        if mode == "local":
            await self.cdp.change_volume("MUTE")
        else:
            pass

    async def on_volume_update(self, data):
        await self.render()
