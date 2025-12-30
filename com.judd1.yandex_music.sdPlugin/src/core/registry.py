from typing import Dict, Type
from src.core.action import Action


ACTION_REGISTRY: Dict[str, Type[Action]] = {}

def action_handler(uuid: str):
    """
    Декоратор, регистрирующий action`ы в глобальном реестре по UUID.
    """
    def decorator(cls_or_func: Type[Action]):
        # from src.core.logger import Logger
        # Logger.info(f"Registering action: {uuid}")  # для отладки
        ACTION_REGISTRY[uuid] = cls_or_func
        return cls_or_func
    return decorator
