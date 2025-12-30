from typing import Any, Callable, TypeVar, Generic
from dataclasses import asdict

T = TypeVar("T")

class SettingsProxy(Generic[T]):
    """
    Оборачивает экземпляры dataclass и вызывает коллбэки при изменении любого атрибута.
    Используется для автоматического сохранения настроек.
    """
    def __init__(self, wrapped: T, on_change: Callable[[T], Any]):
        self.__dict__["_wrapped"] = wrapped
        self.__dict__["_on_change"] = on_change

    def __getattr__(self, name: str) -> Any:
        return getattr(self._wrapped, name)

    def __setattr__(self, name: str, value: Any):
        setattr(self._wrapped, name, value)
        self._on_change(self._wrapped)

    def to_dict(self) -> dict:
        """Вспомогательный метод для получения словаря из обернутого объекта"""
        if hasattr(self._wrapped, "to_dict"):
            return self._wrapped.to_dict()
        return asdict(self._wrapped)
