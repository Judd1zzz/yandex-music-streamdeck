from pydantic import BaseModel, ConfigDict


class YnisonModel(BaseModel):
    model_config = ConfigDict(
        populate_by_name=True,
        extra='allow'
    )
