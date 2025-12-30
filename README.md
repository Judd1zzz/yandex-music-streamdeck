# Yandex Music Integration

<p align="center">
  <a href="README.md">🇬🇧 English</a> | <a href="README_RU.md">🇷🇺 Русский</a>
</p>

<p align="center">
  <img src="com.judd1.yandex_music.sdPlugin/static/img/ym.png" alt="Yandex Music Plugin" width="150">
</p>

<p align="center">
  <b>Control Yandex Music directly from your Stream Deck</b><br>
  Fast • Convenient • No hassle
</p>

<p align="center">
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases"><img src="https://img.shields.io/github/v/release/Judd1zzz/yandex-music-streamdeck?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases"><img src="https://img.shields.io/github/downloads/Judd1zzz/yandex-music-streamdeck/total?style=flat-square&color=green" alt="Downloads"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/stargazers"><img src="https://img.shields.io/github/stars/Judd1zzz/yandex-music-streamdeck?style=flat-square&color=yellow" alt="Stars"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Judd1zzz/yandex-music-streamdeck?style=flat-square" alt="License"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Python-3.10+-3776AB?style=flat-square&logo=python&logoColor=white" alt="Python">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/Stream%20Deck-Compatible-blueviolet?style=flat-square" alt="Stream Deck">
</p>

---

## Compatibility

### Devices

The plugin was developed and tested on **Stream Deck alternatives**: Mirabox, Ajazz (AKP153) and similar devices that use the Ajazz Dock/Stream Dock application.

**Original Elgato Stream Deck**: the plugin launches and displays information correctly (tested on Windows 11, Elgato Stream Deck version 7.0.3). Full testing of all features hasn't been performed yet — if you find bugs, please open an issue.

### Software

The plugin works on **v2** and **v3** versions of the software. To download the latest v3 version:

| Device | Windows | macOS |
|--------|---------|-------|
| **Ajazz** | [ajazz.key123.vip/win](https://ajazz.key123.vip/win) | [ajazz.key123.vip/mac](https://ajazz.key123.vip/mac) |
| **Mirabox / others** | [key123.vip/win](https://key123.vip/win) | [key123.vip/mac](https://key123.vip/mac) |

<details>
<summary><b>Which software to choose?</b></summary>

- If you have **Ajazz** — download from links with `ajazz` in the URL
- If you have **Mirabox** — download Stream Dock (links without `ajazz`)
- **Stream Dock from Mirabox** technically works with Ajazz too:
  - On **Windows** — detects the device normally
  - On **macOS** — sees the device but doesn't connect properly

This happens because Ajazz AKP153 is hardware-wise a clone of Mirabox StreamDock 293S, and the system sees it under that name.

</details>

---

## Features

### Basic Controls
- **Play/Pause** — pause and resume playback
- **Next/Previous** — switch tracks
- **Like/Dislike** — influence "My Vibe" recommendations
- **Mute** — mute audio without losing volume level

### Volume
- **Volume +/-** — adjust volume by 5% per click
- **Volume indicator** — dedicated button showing current level

> **Long press:** smooth adjustment on button hold is implemented in code, but may not work on some Stream Deck alternatives. This appears to be a hardware limitation — the press event only fires when the button is released. On original Stream Deck it should work fine, but hasn't been tested yet.

### Track Info
- **Cover + title + artist** — all on one button
- **Scrolling text** — long titles automatically scroll
- **Copy to clipboard** — click the cover button to copy "Artist - Track" to clipboard
- **Progress bar** — shows remaining time in the track

### Technical Foundation
- **Event-Driven architecture** — instant response, zero CPU load when idle
- **Update-resistant** — `data-test-id` selectors instead of fragile CSS classes
- **Standalone binary** — PyInstaller, no Python or Node.js required

---

## Installation

### 1. Download the plugin

Get the latest release from [Releases](https://github.com/Judd1zzz/yandex-music-streamdeck/releases) and unpack the archive.

---

### 2. Place the plugin folder in the right location

<details>
<summary><img src="https://custom-icon-badges.demolab.com/badge/Windows-0078D6?logo=windows11&logoColor=white" alt="Windows" style="vertical-align: middle;"></summary>

Place the `com.judd1.yandex_music.sdPlugin` folder in the plugins directory.

Press `Win + R`, paste this path and press Enter:
```
%AppData%\HotSpot\StreamDock\plugins
```

</details>

<details>
<summary><img src="https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=F0F0F0" alt="macOS" style="vertical-align: middle;"></summary>

Place the `com.judd1.yandex_music.sdPlugin` folder in the plugins directory.

In Finder press `Cmd + Shift + G` and paste:
```
~/Library/Application Support/HotSpot/StreamDock/plugins
```

> ⚠️ **Important:** Since the plugin is not signed with an Apple Developer ID, macOS puts it in quarantine. After copying, run this command in Terminal:
> ```bash
> xattr -cr ~/Library/Application\ Support/HotSpot/StreamDock/plugins/com.judd1.yandex_music.sdPlugin
> ```
> Without this, the plugin fail to start and show "App is damaged" error.

</details>

---

### 3. Launch Yandex Music client with debug flag

This is crucial. The client must be launched with the `--remote-debugging-port=9222` parameter. Without it, the plugin won't be able to see the application.

Why? Yandex Music client is an Electron app under the hood (essentially a browser). The debug flag opens a port through which the plugin can "communicate" with the app and control it.

<details>
<summary><img src="https://custom-icon-badges.demolab.com/badge/Windows-0078D6?logo=windows11&logoColor=white" alt="Windows" style="vertical-align: middle;"><b> — create a special shortcut</b></summary>

1. Find **Yandex Music** in the Start menu
2. Right-click → **Open file location**
3. Right-click on the file → **Create shortcut**
4. Open shortcut properties (right-click → Properties)
5. In the **Target** field, add at the very end (after the closing quote, with a space):
   ```
   --remote-debugging-port=9222
   ```
   
   It should look something like:
   ```
   "C:\Users\...\Yandex Music.exe" --remote-debugging-port=9222
   ```

6. Click OK and pin this shortcut wherever convenient

From now on, launch music **only through this shortcut**. Regular launch from Start menu won't work — this is important.

</details>

<details>
<summary><img src="https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=F0F0F0" alt="macOS" style="vertical-align: middle;"> <b>— create a wrapper app</b></summary>

You could open Terminal every time and enter the command, but that gets old fast. Better to create a launcher app once.

#### Create the script

1. Open **Script Editor** — find it via Spotlight
2. Paste:
```applescript
do shell script "open -a '/Applications/Yandex Music.app' --args --remote-debugging-port=9222"
```

If your app is named differently (e.g., "Яндекс Музыка.app"), adjust the path.

#### Export as application

1. File → Export...
2. Name: `Yandex Music Debug` (or whatever you prefer)
3. Where: Applications
4. Format: Application
5. Uncheck all checkboxes
6. Save

Now a new launcher will appear in the Applications folder. Launch music through it.

#### 😘 Bonus: nice icon

The new app will have a default script icon. To restore the original Yandex Music logo:

1. Find the original Yandex Music.app in Applications
2. `Cmd + I` → click on the icon in the top-left corner of the window → `Cmd + C`
3. Find your Yandex Music Debug.app
4. `Cmd + I` → click on the icon → `Cmd + V`

Done, now the launcher looks like the original and works as intended.

</details>

---

### 4. Configure the buttons

1. Open Stream Deck application (Ajazz Dock/Stream Dock)
2. Find the **Yandex Music** category in the actions list on the right
3. Drag the buttons you need onto the panel
4. Click on any button — the settings panel at the bottom will show connection status

If the status shows "Connected" — everything works. If not — check that the Yandex Music client is running through the special shortcut/launcher.

---

## Settings

Each button's settings panel allows you to change:

| Parameter | Description |
|-----------|-------------|
| **Control type** | Local (PC client) or Ynison (cloud, beta) |
| **Port** | Connection port to the client (default 9222) |
| **Button style** | Appearance |
| **Display elements** | What to show: cover, title, artist |

---

## Ynison Mode (experimental)

> ⚠️ **This is experimental stuff for those who like to tinker**

Ynison is Yandex's internal protocol for playback synchronization between devices. In theory, it allows controlling music on your phone, Yandex.Station or TV directly from Stream Deck.

### Why "experimental"

In practice, it's more complicated:

- **Yandex Music PC client** completely blocks control via Ynison. There's a [patch](https://github.com/TheKing-OfTime/YandexMusicModPatcher) that partially solves the problem, but it still doesn't work fully. When I tested, the track queue updated, next track displayed correctly, but the actual track switch on PC didn't happen.
- **Mobile clients** (iOS, Android) work normally — Yandex has a reference implementation there.

Essentially, this mode is a demonstration that it's technically possible. When Yandex finishes their protocol, everything will work properly. For now — it's a toy for enthusiasts.

### What's needed to run

1. Start the local API server (`api_for_plugin`)
2. Enter the authorization token in plugin settings

---

## Technical Details

<details>
<summary><b>☁️ Ynison API Server</b></summary>

This is a separate FastAPI server that acts as a proxy between the plugin and the Ynison protocol. The plugin communicates with it via WebSocket and HTTP, and the server maintains the connection with Yandex.

#### Architecture

```
┌─────────────────────┐      WebSocket       ┌───────────────────────┐
│   Stream Deck       │ ◀──────────────────▶ │   api_for_plugin      │
│   Plugin            │      /ws             │   (FastAPI)           │
└─────────────────────┘                      └───────────────────────┘
                                                       │
                                    ┌──────────────────┴──────────────────┐
                                    │                                     │
                                    ▼                                     ▼
                       ┌────────────────────────┐          ┌──────────────────────────┐
                       │   Ynison WebSocket     │          │   Yandex Music REST API  │
                       │   wss://ynison.music.  │          │   api.music.yandex.net   │
                       │   yandex.ru            │          │   (likes, metadata)      │
                       └────────────────────────┘          └──────────────────────────┘
```

#### Components

| File | Purpose |
|------|---------|
| `main.py` | FastAPI application, endpoints `/ws`, `/control/{action}`, `/check_token` |
| `manager.py` | Session manager. `SessionManager` holds active `YnisonSession` for each token |
| `yandex_api.py` | REST client for Yandex Music: likes, dislikes, track metadata |
| `ynison/player.py` | Ynison player implementation: connection, commands, state processing |
| `ynison/client.py` | Low-level WebSocket client for Ynison |
| `ynison/models/` | Pydantic models for serializing all protocol messages |
| `utils/auth.py` | Token and device_id storage for authentication |

#### Endpoints

**WebSocket `/ws`**
- Header `Authorization: <token>`
- Automatically starts Ynison session for this token on connection
- Receives real-time player state updates (JSON)

**POST `/control/{action}`**
- Actions: `play_pause`, `next`, `prev`, `like`, `dislike`
- Header `Authorization: Bearer <token>` or `Authorization: <token>`
- Returns `{"status": "ok"}`

**GET `/check_token`**
- Validates token
- Returns `{"valid": true}` or `{"valid": false}`

#### Strengths

- **Multi-user mode** — one server for multiple users, lazy sessions
- **Metadata enrichment** — Ynison only provides ID, server fetches covers and names via REST
- **Like synchronization** — liked/disliked lists are loaded on session start
- **Fault tolerance** — auto-reconnect, timeouts, graceful shutdown
- **Pydantic models** — entire protocol is typed and validated

#### Launch

```bash
cd api_for_plugin
pip install -r requirements.txt
python main.py
```

Server will start on `http://0.0.0.0:8000`

</details>

<details>
<summary><b>🖥️ How Local Mode Works (CDP)</b></summary>

Local mode uses Chrome DevTools Protocol to control the Yandex Music client directly, without third-party servers.

#### Architecture

```
┌─────────────────────┐                     ┌───────────────────────────────────────┐
│                     │                     │       Yandex Music (Electron)         │
│    Stream Deck      │                     │                                       │
│      Plugin         │                     │   ┌───────────────────────────────┐   │
│     (Python)        │      WebSocket      │   │       injected_api.js         │   │
│                     │ ◀─────────────────▶ │   │   (injected script)           │   │
│                     │  ws://localhost:    │   │                               │   │
│  CDPMediaController │  .../devtools/page  │   │      window.sdNotify() ──────▶│───│──▶ Runtime.bindingCalled
│                     │                     │   │      (callback)               │   │
└─────────────────────┘                     │   └───────────────────────────────┘   │
         │                                  │                                       │
         │ HTTP GET                         │   CDP Debug Port :9222                │
         └─────────────────────────────────▶│   (--remote-debugging-port)           │
           /json (get WS URL)               └───────────────────────────────────────┘
```

#### Data Flow

1. **Connection:**
   - Plugin requests `http://localhost:9222/json` to get WebSocket URL
   - Opens WebSocket connection to the page via CDP
   - Calls `Runtime.addBinding("sdNotify")` to register callback

2. **Script injection:**
   - Plugin injects `injected_api.js` via `Runtime.evaluate`
   - Script creates `window._PyYMController` object
   - Script starts observing via `MutationObserver`

3. **Receiving updates (event-driven):**
   - When player state changes, script calls `window.sdNotify(JSON)`
   - CDP delivers this via `Runtime.bindingCalled` event
   - Plugin parses payload and updates UI

4. **Sending commands:**
   - Plugin calls `Runtime.evaluate` with controller method
   - For example: `_PyYMController.togglePlayPause()`
   - Script finds the right button and emulates click

#### Plugin Components

| File | Purpose |
|------|---------|
| `src/core/cdp.py` | Singleton `CDPMediaController`: connection, RPC, event handling |
| `src/core/scripts/injected_api.js` | JS controller: DOM observation, delta updates, commands |
| `src/core/schemas/states.py` | `dataclass(slots=True)`: `MediaState`, `TrackData`, `PlaybackData` — for speed |
| `src/core/schemas/events.py` | Pydantic models for Stream Deck events (JSON validation needed) |
| `src/actions/*.py` | Buttons: subscribe to events via `register_observer` |

#### Why It's Reliable

- **`data-test-id` selectors** — stable between Yandex Music versions
- **Fallback chains** — if main selector not found, try alternatives
- **Delta updates** — only changed fields are transmitted, not entire state
- **Optimistic UI** — buttons update immediately on press
- **Auto-reconnect** — plugin reconnects automatically on connection loss

</details>

## For Developers

### Local testing without building

If you just want to test or poke around the code — building a binary isn't necessary. Just run the plugin via script:

- **macOS**: `run.sh`
- **Windows**: `run.bat`

In `manifest.json`, specify the corresponding file in `CodePathMac` / `CodePathWin` fields.

### Full build

```bash
git clone https://github.com/Judd1zzz/yandex-music-streamdeck.git
cd yandex-music-streamdeck

python -m venv env
source env/bin/activate  # Windows: env\Scripts\activate

pip install -r requirements.txt
pip install pyinstaller

python tools/build.py
```

The finished plugin will appear in `dist/com.judd1.yandex_music.sdPlugin`.

---

## Problems?

| Symptom | Solution |
|---------|----------|
| Buttons don't respond | Check that the client is running with `--remote-debugging-port=9222` flag |
| Endless "Loading..." | Port 9222 is probably occupied by something else, or client isn't running |
| Purple icons | Restart Stream Deck |
| Long press doesn't work | Probably a limitation of your device (see "Volume" section) |

---

## License

MIT. Do whatever you want.

---

## Acknowledgments

- **[Yandex-Music-Ajazz-Plugin](https://github.com/whxtelxs/Yandex-Music-Ajazz-Plugin)** — thanks to the author for the idea of using `--remote-debugging-port` to connect to the client via CDP.
- **[YandexMusicModPatcher](https://github.com/TheKing-OfTime/YandexMusicModPatcher)** — patch for Ynison support on desktop client.
