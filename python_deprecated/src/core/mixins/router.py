import json
from src.core.routing import EVENT_ROUTING_MAP
from src.core.logger import Logger
import asyncio

class RouterMixin:
    """
    Миксин для класса Plugin, обеспечивающий автоматическую маршрутизацию событий.
    """
    async def route_message(self, message: str):
        """
        Разбирает сообщение, валидирует данные и вызывает соответствующий обработчик.
        
        Логика:
        1. Парсинг JSON и определение типа события.
        2. Валидация данных через Pydantic модели.
        3. Маршрутизация:
           - Если событие действия (Action Event): направляет в конкретный экземпляр Action.
             Обрабатывает жизненный цикл (создает Action при willAppear, удаляет при willDisappear).
           - Если глобальное событие: вызывает метод самого плагина.
        """
        try:
            data = json.loads(message)
            event_name = data.get('event')
            
            if event_name not in EVENT_ROUTING_MAP:
                return

            routing = EVENT_ROUTING_MAP[event_name]
            
            try:
                event_obj = routing.obj_type.model_validate(data)
            except AttributeError:
                event_obj = routing.obj_type.parse_obj(data)

            handler_name = routing.handler_name

            if routing.is_action_event:
                context = getattr(event_obj, 'context', None)
                
                if event_name == "willAppear" and context and context not in self.actions:
                    if hasattr(self, '_handle_new_action'):
                        self._handle_new_action(event_obj)

                if context and context in self.actions:
                    action = self.actions[context]
                    if hasattr(action, handler_name):
                        res = getattr(action, handler_name)(event_obj)
                        if asyncio.iscoroutine(res): await res
                     
                    if event_name == "willDisappear":
                        if context in self.actions:
                            del self.actions[context]
                            if hasattr(self, 'recalculate_ynison_state'):
                                await self.recalculate_ynison_state()
            else:
                if hasattr(self, handler_name):
                    res = getattr(self, handler_name)(event_obj)
                    if asyncio.iscoroutine(res): await res

        except Exception as e:
            Logger.error(f"Routing Error: {e}")
