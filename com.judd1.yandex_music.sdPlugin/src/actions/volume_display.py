from src.core.registry import action_handler
from src.actions.mixins import VolumeObservable
from src.actions.base import YandexMusicBaseAction
from src.core.schemas.events import DidReceiveSettingsModel


@action_handler("com.judd1.yandex_music.action.volume_display")
class VolumeDisplay(VolumeObservable, YandexMusicBaseAction):
    def __init__(self, *args, **kwargs):
        self._last_rendered_icon = None
        super().__init__(*args, **kwargs)

    async def render_action(self):
        vol = 0
        mode = self.get_mode()
        style = self.settings.get("volume_style", "v1")
        if mode == "local":
            if self.cdp.is_connected:
                vol = self.cdp.volume / 100.0
            else:
                await self.set_image(f"btn_yandex_music_vol_level_{style}_0_loading.png")
                self._last_rendered_icon = "loading"
                return
        else:
            if self.client.current_state:
                for dev in self.client.current_state.get("devices", []):
                    if dev.get("info", {}).get("title") == "Deck Player": continue
                    if "volume" in dev: vol = dev["volume"]
                    elif "volume_info" in dev: vol = dev["volume_info"]["volume"]
        
        vol_pct = int(vol * 100) if vol <= 1.0 else int(vol)
        variant = "0"
        if vol_pct == 0: variant = "0"
        elif 1 <= vol_pct <= 29: variant = "1"
        else: variant = "2"
        
        icon_key = f"{style}_{variant}"
        
        if self._last_rendered_icon != icon_key:
            image_name = f"btn_yandex_music_vol_level_{style}_{variant}.png"
            await self.set_image(image_name)
            self._last_rendered_icon = icon_key

        await self.set_title(f"{vol_pct}%")

    async def on_did_receive_settings(self, obj: DidReceiveSettingsModel):
        self._last_rendered_icon = None
        await super().on_did_receive_settings(obj)

    async def on_volume_update(self, data):
        """Перерисовывает при изменении стейта (хэндлер апдейта с CDP)"""
        await self.render()
