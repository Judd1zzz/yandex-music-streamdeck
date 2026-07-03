import json
import unittest
from unittest.mock import MagicMock
from src.core.plugin import Plugin
from src.core.schemas.events import *
from src.core.registry import ACTION_REGISTRY


class MockAction:
    def __init__(self, action, context, settings, plugin, **kwargs):
        self.action_uuid = action
        self.context = context
        self.settings = settings
        self.plugin = plugin
        self.on_will_appear_called = False
        self.on_key_down_called = False

    async def on_will_appear(self, obj: WillAppearModel):
        self.on_will_appear_called = True

    async def on_key_down(self, obj: KeyDownModel):
        self.on_key_down_called = True

ACTION_REGISTRY["com.test.action"] = MockAction

class TestRouting(unittest.IsolatedAsyncioTestCase):
    async def test_routing_flow(self):
        plugin = Plugin(port=1234, plugin_uuid="uuid", event="register", info={})
        plugin.cdp = MagicMock()
        plugin.client = MagicMock()
        plugin.send_json = MagicMock()
        
        wa_payload = {
            "event": "willAppear",
            "action": "com.test.action",
            "context": "ctx_1",
            "device": "dev_1",
            "payload": {
                "settings": {"foo": "bar"},
                "coordinates": {"column": 1, "row": 1},
                "isInMultiAction": False
            }
        }
        await plugin.route_message(json.dumps(wa_payload))
        
        self.assertIn("ctx_1", plugin.active_actions)
        action = plugin.active_actions["ctx_1"]
        self.assertIsInstance(action, MockAction)
        self.assertTrue(action.on_will_appear_called)
        self.assertEqual(action.settings, {"foo": "bar"})
        
        kd_payload = {
            "event": "keyDown",
            "action": "com.test.action",
            "context": "ctx_1",
            "device": "dev_1",
            "payload": {
                "settings": {"foo": "bar"},
                "coordinates": {"column": 1, "row": 1},
                "isInMultiAction": False
            }
        }
        await plugin.route_message(json.dumps(kd_payload))
        self.assertTrue(action.on_key_down_called)


if __name__ == '__main__':
    import json
    unittest.main()
