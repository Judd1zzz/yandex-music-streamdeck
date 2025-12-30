from src.core.types import YnisonCommand
from src.core.registry import action_handler
from src.core.schemas.events import KeyDownModel
from src.actions.mixins import DislikeObservable
from src.actions.base import YandexMusicBaseAction


@action_handler("com.judd1.yandex_music.action.dislike")
class Dislike(DislikeObservable, YandexMusicBaseAction):
    async def on_dislike_update(self, data):
        await self.render()

    async def render_action(self):
        default_image_start = "btn_yandex_music_dislike"
        mode = self.cfg.control_mode
        style = self.cfg.dislike_style
        is_disliked = False
        
        if mode == "local":
            if not self.cdp.is_connected:
                await self.set_image(f"{default_image_start}_{style}_off_loading.png")
                return
            is_disliked = self.cdp.is_disliked
        else:
            if not self.client.is_ready or not self.client.current_track_data:
                await self.set_image(f"{default_image_start}_{style}_off_loading.png")
                return
            track_id = self.client.current_track_data.get("playable_id")
            is_disliked = track_id in self.client.disliked_tracks
        image = f"{default_image_start}_{style}_{'on' if is_disliked else 'off'}.png"
        await self.set_state(1 if is_disliked else 0)
        await self.set_image(image)

    async def on_key_down(self, obj: KeyDownModel):
        mode = self.cfg.control_mode
        if mode == "local":
            await self.cdp.toggle_dislike()
        else:
            await self.client.send_command(YnisonCommand.DISLIKE)
