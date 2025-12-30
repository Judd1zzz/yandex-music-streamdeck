import os
import io
import base64
from typing import Tuple
from PIL import Image, ImageFont
from abc import ABC, abstractmethod


class BaseRenderer(ABC):
    _font_cache = {"title": None, "artist": None}

    @classmethod
    def get_fonts(cls) -> Tuple[ImageFont.ImageFont, ImageFont.ImageFont]:
        """Отложенная загрузка и кэширование шрифтов для оптимизации производительности."""
        if cls._font_cache["title"] and cls._font_cache["artist"]:
            return cls._font_cache["title"], cls._font_cache["artist"]
            
        try:
            if os.name == 'nt': # windows
                 fonts_to_try = [
                     "C:\\Windows\\Fonts\\seguisb.ttf",
                     "C:\\Windows\\Fonts\\segoeui.ttf",
                     "C:\\Windows\\Fonts\\arialbd.ttf",
                     "C:\\Windows\\Fonts\\arial.ttf",
                     "arial.ttf"
                 ]
                 
                 for font_path in fonts_to_try:
                     try:
                         cls._font_cache["title"] = ImageFont.truetype(font_path, 26)
                         cls._font_cache["artist"] = ImageFont.truetype(font_path, 20)
                         break
                     except OSError:
                         continue
                         
            if not cls._font_cache["title"]:
                 try:
                    cls._font_cache["title"] = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 28)
                    cls._font_cache["artist"] = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", 20)
                 except: pass

            if not cls._font_cache["title"]:
                 cls._font_cache["title"] = ImageFont.load_default()
                 cls._font_cache["artist"] = ImageFont.load_default()

        except Exception as e:
            print(f"Font loading error: {e}")
            cls._font_cache["title"] = ImageFont.load_default()
            cls._font_cache["artist"] = ImageFont.load_default()
            
        return cls._font_cache["title"], cls._font_cache["artist"]

    @staticmethod
    def to_base64(image: Image.Image, format: str = "PNG") -> str:
        """Вспомогательный метод для конвертации изображения в строку Base64."""
        buffer = io.BytesIO()
        image.save(buffer, format=format)
        return f"data:image/{format.lower()};base64,{base64.b64encode(buffer.getvalue()).decode('utf-8')}"

    @abstractmethod
    def render(self, *args, **kwargs) -> str:
        pass
