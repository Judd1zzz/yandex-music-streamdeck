from typing import Protocol, Any, Dict
from src.core.schemas.pi import TokenStatusEnum, LocalStatusEnum, TokenStatusEvent, LocalStatusEvent


class ActionSender(Protocol):
    async def send_to_property_inspector(self, payload: Dict[str, Any]): ...


class PIMessenger:
    """
    Вспомогательный класс для отправки строго типизированных сообщений в Property Inspector.
    Оборачивает экземпляр action (или любой объект с методом send_to_property_inspector).
    """
    def __init__(self, action: ActionSender):
        self._action = action

    async def send_token_status(self, status: TokenStatusEnum):
        payload = TokenStatusEvent(status=status).model_dump()
        await self._action.send_to_property_inspector(payload)

    async def send_local_status(self, status: LocalStatusEnum):
        payload = LocalStatusEvent(status=status).model_dump()
        await self._action.send_to_property_inspector(payload)
