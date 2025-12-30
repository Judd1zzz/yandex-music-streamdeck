from src.core.types import YnisonCommand
from src.core.registry import action_handler
from src.actions.mixins import LikeObservable
from src.actions.base import YandexMusicBaseAction
from src.core.schemas.events import KeyDownModel, DidReceiveSettingsModel


@action_handler("com.judd1.yandex_music.action.like")
class Like(LikeObservable, YandexMusicBaseAction):
    async def on_like_update(self, data):
        await self.render()

    async def render_action(self):
        default_image_start = "btn_yandex_music_like"
        mode = self.cfg.control_mode
        style = self.cfg.like_style
        is_liked = False
        
        if mode == "local":
            if not self.cdp.is_connected:
                await self.set_image(f"{default_image_start}_{style}_off_loading.png")
                return
            is_liked = self.cdp.is_liked
        
        else:
            if not self.client.is_ready or not self.client.current_track_data:
                await self.set_image(f"{default_image_start}_{style}_off_loading.png")
                return
            track_id = self.client.current_track_data.get("playable_id")
            is_liked = track_id in self.client.liked_tracks
        
        
        image = f"{default_image_start}_{style}_{'on' if is_liked else 'off'}.png"
        await self.set_state(1 if is_liked else 0)
        await self.set_image(image)

    async def on_key_down(self, obj: KeyDownModel):
        mode = self.cfg.control_mode
        if mode == "local":
            await self.cdp.toggle_like()
        else:
            await self.client.send_command(YnisonCommand.LIKE)
        
    async def on_did_receive_settings(self, obj: DidReceiveSettingsModel):
        await super().on_did_receive_settings(obj)
