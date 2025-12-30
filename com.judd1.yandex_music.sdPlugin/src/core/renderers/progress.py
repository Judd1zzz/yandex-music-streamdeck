from PIL import Image, ImageDraw
from src.core.renderers.base import BaseRenderer


class ProgressRenderer(BaseRenderer):
    @staticmethod
    def format_time(milliseconds):
        """Форматирует миллисекунды в строку времени вида m:ss"""
        if milliseconds is None: return "0:00"
        total_seconds = int(milliseconds // 1000)
        minutes = total_seconds // 60
        seconds = total_seconds % 60
        return f"{minutes}:{seconds:02d}"

    def render(self, current_ms, duration_ms, width=144, height=144, display_mode="stacked"):
        """
        Отрисовывает шкалу прогресса (прогресс-бар) в зависимости от режима отображения.
        """
        image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        draw = ImageDraw.Draw(image)
        title_font, artist_font = self.get_fonts()
        
        current_str = self.format_time(current_ms)
        total_str = self.format_time(duration_ms)
        progress_ratio = (current_ms / duration_ms) if duration_ms > 0 else 0
        progress_ratio = max(0, min(1, progress_ratio))
    
        match display_mode:
            case "stacked":
                curr_bbox = draw.textbbox((0, 0), current_str, font=title_font)
                total_bbox = draw.textbbox((0, 0), total_str, font=artist_font)
                
                curr_w = curr_bbox[2] - curr_bbox[0]
                total_w = total_bbox[2] - total_bbox[0]
                
                draw.text(((width - curr_w) // 2, height // 2 - 30), current_str, font=title_font, fill=(255, 255, 255))
                draw.text(((width - total_w) // 2, height // 2 + 5), total_str, font=artist_font, fill=(180, 180, 180))
    
            case "inline":
                text = f"{current_str} | {total_str}"
                bbox = draw.textbbox((0, 0), text, font=artist_font)
                w = bbox[2] - bbox[0]
                draw.text(((width - w) // 2, (height - 20) // 2), text, font=artist_font, fill=(255, 255, 255))
    
            case "current_only":
                bbox = draw.textbbox((0, 0), current_str, font=title_font)
                w = bbox[2] - bbox[0]
                draw.text(((width - w) // 2, (height - 26) // 2), current_str, font=title_font, fill=(255, 255, 255))
    
            case "total_only":
                bbox = draw.textbbox((0, 0), total_str, font=title_font)
                w = bbox[2] - bbox[0]
                draw.text(((width - w) // 2, (height - 26) // 2), total_str, font=title_font, fill=(255, 255, 255))
    
            case "bar_cli":  # [███░░░]
                bar_len = 12
                filled = int(bar_len * progress_ratio)
                bar_str = "[" + "█" * filled + "░" * (bar_len - filled) + "]"
                
                bbox = draw.textbbox((0, 0), bar_str, font=artist_font)
                w = bbox[2] - bbox[0]
                draw.text(((width - w) // 2, (height - 20) // 2), bar_str, font=artist_font, fill=(255, 208, 0))
    
            case "bar_modern":
                margin = 15
                bar_y = height // 2 + 10
                bar_width = width - 2 * margin
                
                draw.line([(margin, bar_y), (width - margin, bar_y)], fill=(60, 60, 60), width=2)
                
                filled_w = int(bar_width * progress_ratio)
                if filled_w > 0:
                    draw.line([(margin, bar_y), (margin + filled_w, bar_y)], fill=(255, 208, 0), width=4)
                    dot_r = 5
                    draw.ellipse([
                        (margin + filled_w - dot_r, bar_y - dot_r), 
                        (margin + filled_w + dot_r, bar_y + dot_r)
                    ], fill=(255, 255, 255))
    
                draw.text((margin, bar_y - 30), current_str, font=artist_font, fill=(255, 255, 255))
                total_bbox = draw.textbbox((0, 0), total_str, font=artist_font)
                total_w = total_bbox[2] - total_bbox[0]
                draw.text((width - margin - total_w, bar_y - 30), total_str, font=artist_font, fill=(180, 180, 180))
    
        return self.to_base64(image)
