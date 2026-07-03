import os
import sys
import asyncio
import argparse
import tempfile
import signal
from src.core import Plugin, Logger


def ensure_single_instance():
    """
    Гарантирует, что работает только один экземпляр плагина.
    Если найден старый процесс (zombie) — убивает его.
    """
    lock_file = os.path.join(tempfile.gettempdir(), 'ym_streamdeck_plugin.pid')
    
    if os.path.exists(lock_file):
        try:
            with open(lock_file, 'r') as f:
                old_pid_str = f.read().strip()
                if old_pid_str.isdigit():
                    old_pid = int(old_pid_str)
                    if old_pid != os.getpid():
                        try:
                            Logger.info(f"Found old instance (PID {old_pid}). Killing it...")
                            os.kill(old_pid, signal.SIGTERM)
                        except OSError:
                            pass
        except Exception as e:
            Logger.warn(f"Failed to check lock file: {e}")

    try:
        with open(lock_file, 'w') as f:
            f.write(str(os.getpid()))
    except Exception as e:
        Logger.error(f"Failed to write PID file: {e}")


async def main():
    Logger.info("Plugin Start")
    ensure_single_instance()
    
    parser = argparse.ArgumentParser(description='Stream Dock Plugin')
    parser.add_argument('-port', type=int, required=True, help='WebSocket port number')
    parser.add_argument('-pluginUUID', type=str, required=True, help='Unique identifier for the plugin')
    parser.add_argument('-registerEvent', type=str, required=True, help='Event type for plugin registration')
    parser.add_argument('-info', type=str, required=True, help='JSON string containing Stream Dock and device information')
    args = parser.parse_args()

    try:
        plugin = Plugin(args.port, args.pluginUUID, args.registerEvent, args.info)
        await plugin.run()
    except Exception as e:
        Logger.error(f"Plugin Main Error: {e}")
        sys.exit(0)


if __name__ == '__main__':
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    finally:
        pass
