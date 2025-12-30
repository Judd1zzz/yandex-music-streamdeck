import os
import sys
import logging
from typing import Optional
from logging.handlers import RotatingFileHandler


class Logger:
    """Логгер. Нечего добавить."""
    
    _instance: Optional['Logger'] = None
    _logger: Optional[logging.Logger] = None
    
    def __new__(cls) -> 'Logger':
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._setup_logger()
        return cls._instance
    
    @classmethod
    def get_instance(cls) -> 'Logger':
        if cls._instance is None:
            cls._instance = Logger()
        return cls._instance
    
    @classmethod
    def _setup_logger(cls):
        if cls._logger is None:
            cls._logger = logging.getLogger('StreamDock')
            cls._logger.setLevel(logging.INFO)
            
            if getattr(sys, 'frozen', False):
                base_path = os.path.join(os.path.dirname(sys.executable), 'logs')
            else:
                base_path = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(__file__))), 'logs')
            
            try:
                os.makedirs(base_path, exist_ok=True)
                
                log_file = os.path.join(base_path, 'plugin.log')
                
                handler = RotatingFileHandler(log_file, maxBytes=2*1024*1024, backupCount=3, encoding='utf-8')
                handler.setFormatter(logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s'))
                cls._logger.addHandler(handler)
                
                console_handler = logging.StreamHandler()
                console_handler.setFormatter(logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s'))
                cls._logger.addHandler(console_handler)
            except Exception as e:
                print(f"Failed to setup file handler: {e}")
                console_handler = logging.StreamHandler()
                console_handler.setFormatter(logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s'))
                cls._logger.addHandler(console_handler)
    
    @classmethod
    def get_logger(cls) -> logging.Logger:
        if cls._logger is None:
            cls._setup_logger()
        return cls._logger
    
    @classmethod
    def info(cls, message: str):
        cls.get_instance().get_logger().info(message)
    
    @classmethod
    def error(cls, message: str):
        cls.get_instance().get_logger().error(message)
    
    @classmethod
    def warning(cls, message: str):
        cls.get_instance().get_logger().warning(message)
    
    @classmethod
    def debug(cls, message: str):
        cls.get_instance().get_logger().debug(message)
