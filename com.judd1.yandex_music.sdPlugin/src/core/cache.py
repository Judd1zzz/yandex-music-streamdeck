import threading
from typing import Optional, Any
from collections import OrderedDict


class ImageCache:
    """Кэширует обработанные обложки треков (реализует lru-стратегию)"""
    def __init__(self, capacity: int = 10):
        self.capacity = capacity
        self.cache = OrderedDict()
        self.lock = threading.Lock()

    def get(self, key: str) -> Optional[Any]:
        """
        Извлекает изображение из кэша.
        Перемещает запись в конец очереди, помечая её как недавно использованную.
        """
        with self.lock:
            if key not in self.cache:
                return None
            self.cache.move_to_end(key)
            return self.cache[key]

    def put(self, key: str, value: Any) -> None:
        """Добавляет изображение, удаляя самое старое при переполнении."""
        with self.lock:
            if key in self.cache:
                self.cache.move_to_end(key)
            self.cache[key] = value
            if len(self.cache) > self.capacity:
                self.cache.popitem(last=False)

    def clear(self):
        with self.lock:
            self.cache.clear()


_image_cache = ImageCache(capacity=10)


def get_image_cache() -> ImageCache:
    return _image_cache


class StaticAssetCache:
    """
    Кэширует статические ресурсы (иконки).
    Использует простой словарь (без ротации).
    
    Формат хранения:
    {
        'filename': Base64_String
    }
    """
    def __init__(self):
        self.cache = {}
        self.lock = threading.Lock()

    def get(self, key: str) -> Optional[str]:
        with self.lock:
            return self.cache.get(key)

    def put(self, key: str, value: str) -> None:
        with self.lock:
            self.cache[key] = value

_static_cache = StaticAssetCache()

def get_static_cache() -> StaticAssetCache:
    return _static_cache
