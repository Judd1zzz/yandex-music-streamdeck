import json
import asyncio
import logging
from typing import Dict, Set
from manager import SessionManager
from contextlib import asynccontextmanager
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Request, Header, HTTPException


logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] [API] %(message)s',
    datefmt='%H:%M:%S'
)
logger = logging.getLogger("API")
manager = SessionManager()
connected_websockets: Dict[str, Set[WebSocket]] = {}


async def on_state_update(token, state):
    async def broadcast():
        if token in connected_websockets:
            sockets = connected_websockets[token]
            logger.info(f"Broadcasting update to {len(sockets)} clients for token {token[:5]}...")
            try:
                msg = json.dumps(state) if isinstance(state, dict) else str(state)
            except Exception as e:
                logger.error(f"Failed to JSON serialize state: {e}")
                msg = str(state)
                
            dead_sockets = set()
            for ws in list(sockets):
                try:
                    await asyncio.wait_for(ws.send_text(msg), timeout=1.5)
                    logger.debug(f"Successfully sent update to WS {id(ws)}")
                except Exception as e:
                    logger.warning(f"Failed to send to WS for {token[:5]}: {e}")
                    dead_sockets.add(ws)
            
            for ws in dead_sockets:
                connected_websockets[token].discard(ws)
        else:
            logger.debug(f"No clients connected for token {token[:5]}.. skipping broadcast.")
    asyncio.create_task(broadcast())


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Starting Multi-User API Service...")
    manager.on_global_update = on_state_update
    yield
    logger.info("Shutting down API Service...")
    await manager.shutdown()


app = FastAPI(lifespan=lifespan)


@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    token = websocket.headers.get("Authorization")
    if not token:
        logger.warning("WS Connection attempt without token")
        await websocket.close(code=4003)
        return

    await websocket.accept()
    
    if token not in connected_websockets:
        connected_websockets[token] = set()
    connected_websockets[token].add(websocket)
    
    logger.info(f"WS Client connected: {token[:5]}..")
    
    try:
        try:
             session = await manager.get_session(token)
        except Exception as e:
             logger.error(f"Session init failed for {token[:5]}: {e}")
             await websocket.close(code=4001)
             return

        if session.ynison and session.ynison.state:
             try:
                 initial_msg = session.ynison.state.model_dump_json(by_alias=True)
                 await asyncio.wait_for(websocket.send_text(initial_msg), timeout=2.0)
             except Exception as e:
                 logger.warning(f"Failed to send initial state to WS: {e}")
             
        while True:
            await websocket.receive_text()
            
    except WebSocketDisconnect:
        logger.info(f"WS Client disconnected: {token[:5]}..")
        if token in connected_websockets:
            connected_websockets[token].discard(websocket)
            if not connected_websockets[token]:
                del connected_websockets[token]


@app.post("/control/{action}")
async def control(action: str, authorization: str = Header(None), token: str = None):
    user_token = token
    if not user_token and authorization:
         if authorization.startswith("Bearer "):
             user_token = authorization.split(" ")[1]
         else:
             user_token = authorization
             
    if not user_token:
        raise HTTPException(status_code=401, detail="Token required")
        
    session = await manager.get_session(user_token)
    
    match action:
        case "play_pause":
            await session.play_pause()
        case "next":
            await session.next()
        case "prev":
            await session.prev()
        case "like":
            await session.like()
        case "dislike":
            await session.dislike()
        case _:
            return {"error": "unknown action"}
    return {"status": "ok"}


@app.get("/check_token")
async def check_token(request: Request):
    token = request.headers.get("Authorization")
    if not token or len(token) < 5:
        return {"valid": False}
    
    try:
        await manager.get_session(token)
        return {"valid": True}
    except Exception as e:
        logger.error(f"Token validation failed: {e}")
        return {"valid": False}


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
