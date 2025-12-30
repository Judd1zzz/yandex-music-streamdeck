import io
import os
from PIL import Image, ImageDraw
from src.core.renderers.base import BaseRenderer


class InfoRenderer(BaseRenderer):
    def render(self, cover_data, title, artists, marquee_offset=0, width=144, height=144, corner_radius=20, cached_cover_img=None, show_cover=True, show_title=True, show_artists=True):
        """
        Рендерит информацию о треке (обложка, название, исполнитель) с учетом флагов отображения.
        
        Особенности:
        - Если включена обложка, накладывает затемняющий градиент для читаемости текста.
        - Если текст не помещается по ширине, реализует эффект 'бегущей строки' (marquee), используя offset.
        - Применяет маску для скругления углов.

        Returns:
            Tuple[str, bool]: (base64_string, needs_animation)
            - needs_scrolling (bool): указывает, требуется ли дальнейшая анимация текста.
        """
        if title is None: title = ""
        if artists is None: artists = ""
        
        image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        has_cover = show_cover
    
        if has_cover:
            cover_img = cached_cover_img
            if not cover_img and cover_data:
                try:
                    raw_cover = Image.open(io.BytesIO(cover_data)).convert("RGBA")
                    cover_img = raw_cover.resize((width, height), Image.LANCZOS)
                except Exception:
                     cover_img = None
    
            if not cover_img:
                 current_dir = os.path.dirname(os.path.abspath(__file__))
                 root_dir = os.path.abspath(os.path.join(current_dir, "../../../"))
                 emptiness_path = os.path.join(root_dir, "static/img", "emptiness_black.png")
                 
                 if os.path.exists(emptiness_path):
                     cover_img = Image.open(emptiness_path).convert("RGBA").resize((width, height), Image.LANCZOS)
                 else:
                     cover_img = Image.new("RGBA", (width, height), (0, 0, 0, 255))
    
            image.paste(cover_img, (0, 0), cover_img)

            if show_title or show_artists:
                overlay = Image.new("RGBA", (width, height), (0,0,0,0))
                overlay_draw = ImageDraw.Draw(overlay)
                for y in range(int(height * 0.4), height):
                    alpha = int(240 * (y - height * 0.4) / (height * 0.6))
                    overlay_draw.line([(0, y), (width, y)], fill=(0, 0, 0, alpha))
                image = Image.alpha_composite(image, overlay)

        draw = ImageDraw.Draw(image)
        title_font, artist_font = self.get_fonts()
        title_color = (255, 255, 255, 255)
        artist_color = (180, 180, 180, 255)
        
        if has_cover:
            title_y = int(height * 0.61)
            artist_y = int(height * 0.81)
            if show_title and not show_artists: title_y = int(height * 0.72)
            if not show_title and show_artists: artist_y = int(height * 0.72)
        else:
            if show_title and show_artists:
                total_text_height = 56
                start_y = (height - total_text_height) // 2
                title_y = start_y
                artist_y = start_y + 36
            else:
                if show_title: title_y = (height - 26) // 2
                if show_artists: artist_y = (height - 20) // 2
    
        safe_width = width - 10
    
        needs_scrolling = False
        if show_title:
            title_bbox = draw.textbbox((0, 0), str(title), font=title_font)
            title_w = title_bbox[2] - title_bbox[0]
            if title_w > safe_width:
                needs_scrolling = True
                gap, cycle_len = 50, title_w + 50
                x = 5 - (marquee_offset % cycle_len)
                draw.text((x, title_y), str(title), font=title_font, fill=title_color)
                if x + title_w < width:
                     draw.text((x + cycle_len, title_y), str(title), font=title_font, fill=title_color)
            else:
                x = (width - title_w) // 2
                draw.text((x, title_y), str(title), font=title_font, fill=title_color)
    
        if show_artists:
            artist_bbox = draw.textbbox((0, 0), str(artists), font=artist_font)
            artist_w = artist_bbox[2] - artist_bbox[0]
            if artist_w > safe_width:
                needs_scrolling = True
                gap, cycle_len = 50, artist_w + 50
                x = 5 - ((marquee_offset * 0.8) % cycle_len)
                draw.text((x, artist_y), str(artists), font=artist_font, fill=artist_color)
                if x + artist_w < width:
                     draw.text((x + cycle_len, artist_y), str(artists), font=artist_font, fill=artist_color)
            else:
                x = (width - artist_w) // 2
                draw.text((x, artist_y), str(artists), font=artist_font, fill=artist_color)
    
        if has_cover and corner_radius > 0:
             mask = Image.new("L", (width, height), 0)
             d = ImageDraw.Draw(mask)
             d.rounded_rectangle([(0,0), (width, height)], radius=corner_radius, fill=255)
             image.putalpha(mask)
    
        return self.to_base64(image), needs_scrolling
