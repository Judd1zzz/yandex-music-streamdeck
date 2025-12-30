from typing import Optional
from .base import YnisonModel
from .common import YnisonKeepAliveParams


class YnisonRedirect(YnisonModel):
    host: str
    redirect_ticket: str
    session_id: str
    keep_alive_params: Optional[YnisonKeepAliveParams] = None
