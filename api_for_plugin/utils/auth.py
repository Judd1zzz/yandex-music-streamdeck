from dataclasses import dataclass


@dataclass
class AuthStorage:
    token: str
    user_id: str = "0"
    device_id: str = "python"
