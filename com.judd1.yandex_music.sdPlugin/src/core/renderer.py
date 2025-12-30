import io
import base64
import aiohttp
from typing import Optional, Tuple
from PIL import Image, ImageDraw, ImageFont, ImageEnhance


TARGET_SIZE = (72, 72) # дэфолтный размер кнопки (std)
FONT_SIZE_TITLE = 11
FONT_SIZE_ARTIST = 9
TEXT_COLOR = (255, 255, 255)
BG_COLOR = (0, 0, 0)

def image_to_base64(img: Image, format: str = "JPEG", quality: int = 85) -> str:
    """
    Конвертирует объект PIL Image в строку Base64, оптимизированную для Stream Deck.
    Использует JPEG для обложек (фото) и PNG для иконок (прозрачность).
    """
    buffer = io.BytesIO()
    
    if format.upper() == "JPEG":
        if img.mode in ("RGBA", "P"):
            img = img.convert("RGB")
        img.save(buffer, format="JPEG", quality=quality, optimize=True)
        prefix = "data:image/jpeg;base64,"
    else:
        img.save(buffer, format="PNG", optimize=True)
        prefix = "data:image/png;base64,"
        
    img_bytes = buffer.getvalue()
    base64_str = base64.b64encode(img_bytes).decode('utf-8')
    return prefix + base64_str

def draw_button_image(
    base_image: Optional[Image.Image], 
    icon_overlay: Optional[Image.Image],
    title: str,
    artist: str,
    animation_offset: int = 0
) -> Tuple[str, bool]:
    """
    Компоновка изображения кнопки
    
    Returns:
        Tuple[str, bool]: (base64_string, needs_animation)
    """
    needs_animation = False
    
    if base_image:
        if base_image.size != TARGET_SIZE:
             img = base_image.resize(TARGET_SIZE, Image.Resampling.BILINEAR)
        else:
             img = base_image.copy()
        
        if title or artist:
            enhancer = ImageEnhance.Brightness(img)
            img = enhancer.enhance(0.4) 
    else:
        img = Image.new('RGB', TARGET_SIZE, color=BG_COLOR)

    if icon_overlay:
        icon_w, icon_h = icon_overlay.size
        if icon_w > 40 or icon_h > 40:
             icon_overlay.thumbnail((40, 40), Image.Resampling.BILINEAR)
             icon_w, icon_h = icon_overlay.size
             
        x = (TARGET_SIZE[0] - icon_w) // 2
        y = (TARGET_SIZE[1] - icon_h) // 2
        
        if icon_overlay.mode == 'RGBA':
             img.paste(icon_overlay, (x, y), icon_overlay)
        else:
             img.paste(icon_overlay, (x, y))

    if title or artist:
        draw = ImageDraw.Draw(img)
        try:
             font_title = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", FONT_SIZE_TITLE, index=0)
             font_artist = ImageFont.truetype("/System/Library/Fonts/Helvetica.ttc", FONT_SIZE_ARTIST, index=0)
        except:
             font_title = ImageFont.load_default()
             font_artist = ImageFont.load_default()

        bbox = draw.textbbox((0, 0), title, font=font_title)
        text_width = bbox[2] - bbox[0]
        
        # макс. ширина с отступом (72 - 4 = 68)
        MAX_WIDTH = 68
        
        if text_width > MAX_WIDTH:
            needs_animation = True
            display_text = f"{title}   {title}"
            full_bbox = draw.textbbox((0, 0), display_text, font=font_title)
            full_width = full_bbox[2] - full_bbox[0]
            x_pos = 36 - (animation_offset % (text_width + 15)) # 15 px gap
            x_effective = 2 - animation_offset
            
            loop_width = text_width + 20 # gap
            effective_offset = animation_offset % loop_width
            
            draw.text((2 - effective_offset, 15), title, font=font_title, fill=TEXT_COLOR, anchor="lm")
            draw.text((2 - effective_offset + loop_width, 15), title, font=font_title, fill=TEXT_COLOR, anchor="lm")
            
        else:
            draw.text((36, 15), title, font=font_title, fill=TEXT_COLOR, anchor="mm")
        draw.text((36, 55), artist, font=font_artist, fill=(200, 200, 200), anchor="mm")

    return image_to_base64(img, format="JPEG" if base_image else "PNG"), needs_animation


async def fetch_image(session: aiohttp.ClientSession, url: str) -> Optional[Image.Image]:
    """Вспомогательная функция для загрузки изображения и возврата объекта PIL"""
    if not url: return None
    try:
        async with session.get(url, timeout=2) as resp:
            if resp.status == 200:
                data = await resp.read()
                return Image.open(io.BytesIO(data))
    except:
        return None
    return None
