import unittest
from src.core.renderers.info import InfoRenderer
from src.core.renderers.progress import ProgressRenderer


class TestRenderers(unittest.TestCase):
    def test_info_renderer_basic(self):
        renderer = InfoRenderer()
        b64, scroll = renderer.render(None, "Title", "Artist")
        self.assertTrue(b64.startswith("data:image/png;base64,"))
        self.assertIsInstance(scroll, bool)

    def test_progress_renderer_stacked(self):
        renderer = ProgressRenderer()
        b64 = renderer.render(50000, 100000, display_mode="stacked")
        self.assertTrue(b64.startswith("data:image/png;base64,"))

    def test_progress_renderer_bar_modern(self):
        renderer = ProgressRenderer()
        b64 = renderer.render(50000, 100000, display_mode="bar_modern")
        self.assertTrue(b64.startswith("data:image/png;base64,"))
    
    def test_progress_time_format(self):
        renderer = ProgressRenderer()
        self.assertEqual(renderer.format_time(61000), "1:01")
        self.assertEqual(renderer.format_time(0), "0:00")
        self.assertEqual(renderer.format_time(None), "0:00")


if __name__ == '__main__':
    unittest.main()
