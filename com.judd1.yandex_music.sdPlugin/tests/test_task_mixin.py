import asyncio
import unittest
from src.core.mixins.task import BackgroundTaskMixin


class MockAction(BackgroundTaskMixin):
    pass


class TestBackgroundTaskMixin(unittest.IsolatedAsyncioTestCase):
    async def test_start_and_cancel_task(self):
        obj = MockAction()
        
        async def sleeper():
            try:
                await asyncio.sleep(10)
            except asyncio.CancelledError:
                pass
        
        obj.start_task("test", sleeper())
        self.assertIn("test", obj._bg_tasks)
        task = obj._bg_tasks["test"]
        self.assertFalse(task.done())
        
        obj.cancel_task("test")
        
        self.assertNotIn("test", obj._bg_tasks)
        
        await asyncio.sleep(0) 
        self.assertTrue(task.done() or task.cancelled())

    async def test_auto_cleanup_on_completion(self):
        obj = MockAction()
        
        async def quick_task():
            return "done"
            
        obj.start_task("quick", quick_task())
        self.assertIn("quick", obj._bg_tasks)
        
        await asyncio.sleep(0.1)
        
        self.assertNotIn("quick", obj._bg_tasks)

    async def test_cancel_all_tasks(self):
        obj = MockAction()
        
        async def sleeper():
             try: await asyncio.sleep(10)
             except: pass
             
        obj.start_task("t1", sleeper())
        obj.start_task("t2", sleeper())
        
        self.assertEqual(len(obj._bg_tasks), 2)
        tasks = list(obj._bg_tasks.values())
        
        obj.cancel_all_tasks()
        
        self.assertEqual(len(obj._bg_tasks), 0)
        
        await asyncio.sleep(0)
        for t in tasks:
            self.assertTrue(t.done() or t.cancelled())


if __name__ == '__main__':
    unittest.main()
