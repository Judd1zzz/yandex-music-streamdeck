from typing import Dict, Type, get_type_hints
from pydantic import BaseModel
from src.core.schemas.events import BaseEventModel
from src.core.event_handlers import ActionEventHandlersMixin, PluginEventHandlersMixin

class EventRoutingObj(BaseModel):
    handler_name: str
    obj_type: Type[BaseEventModel]
    is_action_event: bool

EVENT_ROUTING_MAP: Dict[str, EventRoutingObj] = {}

def _populate_map(mixin: Type, is_action: bool):
    for name, method in mixin.__dict__.items():
        if name.startswith("on_") and callable(method):
            try:
                type_hints = get_type_hints(method)
                if "obj" in type_hints:
                    model_class = type_hints["obj"]
                    if issubclass(model_class, BaseEventModel):
                        event_name = getattr(model_class, "event", None)
                        if not isinstance(event_name, str):
                             try:
                                 if hasattr(model_class, "model_fields"):
                                    event_name = model_class.model_fields['event'].default
                                 else:
                                    event_name = model_class.__fields__['event'].default
                             except:
                                 pass
                        
                        if event_name:
                            EVENT_ROUTING_MAP[event_name] = EventRoutingObj(
                                handler_name=name,
                                obj_type=model_class,
                                is_action_event=is_action
                            )
            except Exception as e:
                print(f"Failed to inspect {name}: {e}")


_populate_map(ActionEventHandlersMixin, is_action=True)
_populate_map(PluginEventHandlersMixin, is_action=False)
