import asyncio
from src.core.logger import Logger
from typing import Dict, Coroutine

class BackgroundTaskMixin:
    """
    Миксин для управления фоновыми задачами asyncio.
    Обеспечивает отслеживание задач и их корректную отмену.
    """
    _bg_tasks: Dict[str, asyncio.Task]

    def start_task(self, name: str, coro: Coroutine):
        """
        Запускает новую фоновую задачу, отменяя предыдущую с тем же именем.
        """
        if not hasattr(self, "_bg_tasks"):
            self._bg_tasks = {}

        self.cancel_task(name)
        
        task = asyncio.create_task(coro)
        self._bg_tasks[name] = task
        
        def _done(t):
            if name in self._bg_tasks and self._bg_tasks[name] == t:
                del self._bg_tasks[name]
        
        task.add_done_callback(_done)
    
    def cancel_task(self, name: str):
        """Отменяет указанную задачу по имени."""
        if not hasattr(self, "_bg_tasks"): return
        
        if name in self._bg_tasks:
            task = self._bg_tasks[name]
            if not task.done():
                task.cancel()
            del self._bg_tasks[name]

    def cancel_all_tasks(self):
        """Отменяет все отслеживаемые задачи."""
        if not hasattr(self, "_bg_tasks"): return
        
        for name in list(self._bg_tasks.keys()):
            self.cancel_task(name)
