import unittest
from unittest.mock import AsyncMock
from src.core.pi_communicator import PIMessenger
from src.core.schemas.pi import TokenStatusEnum, LocalStatusEnum


class MockAction:
    def __init__(self):
        self.send_to_property_inspector = AsyncMock()


class TestPIComms(unittest.IsolatedAsyncioTestCase):
    async def test_send_token_status(self):
        action = MockAction()
        messenger = PIMessenger(action)
        
        await messenger.send_token_status(TokenStatusEnum.VALID)
        
        action.send_to_property_inspector.assert_called_with({
            "event": "TokenStatus",
            "status": "valid"
        })

    async def test_send_local_status(self):
        action = MockAction()
        messenger = PIMessenger(action)
        
        await messenger.send_local_status(LocalStatusEnum.CONNECTED)
        
        action.send_to_property_inspector.assert_called_with({
            "event": "LocalStatus",
            "status": "connected"
        })

if __name__ == '__main__':
    unittest.main()
