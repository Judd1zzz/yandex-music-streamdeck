import unittest
from dataclasses import dataclass
from src.core.settings_manager import SettingsProxy


@dataclass
class MockSettings:
    foo: str = "bar"
    count: int = 0
    
    def to_dict(self):
        from dataclasses import asdict
        return asdict(self)


class TestSettingsProxy(unittest.TestCase):
    def test_proxy_access(self):
        s = MockSettings()
        called = False
        def cb(obj): nonlocal called; called = True
        
        proxy = SettingsProxy(s, cb)
        
        self.assertEqual(proxy.foo, "bar")
        self.assertEqual(proxy.count, 0)
        
        proxy.foo = "baz"
        self.assertEqual(s.foo, "baz")
        self.assertTrue(called)

    def test_to_dict(self):
        s = MockSettings()
        proxy = SettingsProxy(s, lambda x: None)
        
        d = proxy.to_dict()
        self.assertEqual(d, {"foo": "bar", "count": 0})


if __name__ == '__main__':
    unittest.main()
