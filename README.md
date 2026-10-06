# Yandex Music Integration

<p align="center">
  <a href="README.md">🇬🇧 English</a> | <a href="README_RU.md">🇷🇺 Русский</a>
</p>

![](assets/main.png)

<p align="center">
  <b>Control Yandex Music directly from your Stream Deck</b><br>
  Fast • Convenient • No hassle
</p>

<p align="center">
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases"><img src="https://img.shields.io/github/v/release/Judd1zzz/yandex-music-streamdeck?style=flat-square&color=blue" alt="Release"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases"><img src="https://img.shields.io/github/downloads/Judd1zzz/yandex-music-streamdeck/total?style=flat-square&color=green" alt="Downloads"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/stargazers"><img src="https://img.shields.io/github/stars/Judd1zzz/yandex-music-streamdeck?style=flat-square&color=yellow" alt="Stars"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-PolyForm--Strict-8B78E6?style=flat-square" alt="License"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Backend-Rust-CE412B?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/Stream%20Deck-Compatible-blueviolet?style=flat-square" alt="Stream Deck">
</p>

<p align="center">
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases/latest"><img src="https://custom-icon-badges.demolab.com/badge/Download_for_Windows-0078D6?style=for-the-badge&logo=windows11&logoColor=white" alt="Download for Windows"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases/latest"><img src="https://img.shields.io/badge/Download_for_macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS"></a>
</p>

<p align="center">
  <a href="https://marketplace.elgato.com/product/yandex-music-integration-43741c24-1784-4492-be32-c631d7c55829"><img src="https://docs.elgato.com/img/badges/get-it-on-marketplace--dark.svg" alt="Get it on Elgato Marketplace" height="40"></a>
  <a href="https://space.key123.vip/product/20260706002752"><img src="assets/badges/get-it-on-streamdock--dark.svg" alt="Get it on StreamDock Store" height="40"></a>
</p>

<p align="center">
  <sub>An unofficial, fan-made project. Not affiliated with, endorsed by, or supported by Yandex or the Yandex Music service. All trademarks belong to their respective owners.</sub>
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

<details>
<summary><b>For owners of the Russian-market device</b></summary>

The manual that ships with the device often lists links like `ajazz.key123.vip/RUSwin`. If the link ends with **RUS** — you'll download an outdated version of the software.

Use the regular links without RUS. On first launch the software will offer to switch the language.

</details>

---

## Security

> ⚠️ **In short: simply using plugins does not put your account at risk by itself. The risk appears only if a plugin's author is dishonest: access to the client makes it possible to steal your login data. Choose trusted software — and be especially careful with plugins that have auto-update.**

**Why plugins work through the debug port.** Yandex Music has no official API or SDK for its desktop client — requests for Stream Deck support have been around for almost seven years and remain unanswered. That is why plugins for it rely on the Chrome DevTools debug port to work with the client directly — and this one did too, up to and including v2.4.0. Through years of inaction Yandex is effectively pushing users towards software built on debug access — including potentially dangerous software.

**What the risk is.** The debug port is a general-purpose control channel: Yandex Music is an Electron app (essentially a browser), and through that port software can not only press player buttons but also, for example, pull the cookies and tokens of your Yandex account. The port is local-only and unreachable from outside, but anything running on your computer can connect to it and do the same things the plugin does.

**How this plugin works since v2.5.0.** This plugin no longer needs an open debug port. It ships a signed bridge library, `ym-bridge`, that talks to Yandex Music over a private channel that exists only between the plugin and the client: nothing listens on a port, so nothing else on your machine can reach the client through it. Both sides verify each other — the plugin refuses to load a bridge that isn't signed by my key, and the bridge refuses to serve anything except my plugin. The check is fail-closed: an unverified file is never loaded.

**Why auto-update is a special case.** A plugin with auto-update can deliver arbitrary code to your computer at any moment. Even if it is harmless today, one update is all a malicious author needs to steal accounts at scale. An unverified plugin with auto-update is the worst combination: you end up trusting not the code you installed, but the author — forever.

**About copies of this plugin.** My plugin's source code is publicly visible, and it is precisely this project that others now regularly borrow ideas and functionality from: there are documented cases of specific features being lifted wholesale. A copy may look familiar, but it goes through no external review — nobody except its author knows what its next update will bring.

**Where I stand.** The plugin's source code is open for reading and audit, except the bridge: the `ym-bridge` library, the code it injects into the client and the Windows injector ship ready-built and signed. Since v2.5.0 every release ships the bridge library signed with the same hardware key as the releases, and the plugin checks that signature before loading it.

**Why the bridge is closed.** Not to hide anything from you. My plugin has already been copied. The debug-port problem described above is exactly why I don't want copies of the bridge to multiply: a forked bridge is the perfect place to plant malicious code, and from the outside it would look just like mine. Closed source here is protection against careless copying and nothing more — the bridge is not virtualized with protectors like VMProtect, so specialists are welcome to take `ym-bridge.dll` / `ym-bridge.dylib` apart and see what it does. Anyone able to reverse-engineer it is competent enough to write their own anyway.

Then there is the [Elgato Marketplace](https://marketplace.elgato.com/product/yandex-music-integration-43741c24-1784-4492-be32-c631d7c55829) build: every version of that build is reviewed by Elgato before it is published — you can't just walk onto the Marketplace, and that is one more layer of protection. That build has no self-updater at all: updates are delivered by the store itself. And the GitHub build's auto-update accepts only releases signed with my hardware key — here is how to verify that yourself.

### Verifying a release

Starting with v2.4.0 I personally sign every GitHub release: the `SHA256SUMS` file with the hashes of both archives is signed with my hardware key — a signature that cannot be produced without physically touching the key, even if my computer or GitHub account were compromised. The plugin's auto-update verifies this signature on its own and silently rejects anything not signed by me. To check manually (`SHA256SUMS` and `SHA256SUMS.sig` ship with every release next to the archives):

```bash
curl -sO https://raw.githubusercontent.com/Judd1zzz/yandex-music-streamdeck/main/allowed_signers
ssh-keygen -Y verify -f allowed_signers -I Judd1zzz -n file -s SHA256SUMS.sig < SHA256SUMS
shasum -a 256 -c SHA256SUMS --ignore-missing
```

The first check should print `Good "file" signature for Judd1zzz with ED25519-SK key`, the second — `OK` next to the archive you downloaded (on Windows use `sha256sum` from Git Bash instead of `shasum`). You'll need OpenSSH 8.2 or newer (the one built into Windows 10 won't do — use the one that ships with Git for Windows).

The bridge library is signed with the same key. To check it, unpack the archive next to `allowed_signers` and run (on Windows use `ym-bridge.dll` and `ym-bridge.dll.sig`):

```bash
ssh-keygen -Y verify -f allowed_signers -I Judd1zzz -n file -s com.judd1.yandex_music.sdPlugin/bin/ym-bridge.dylib.sig < com.judd1.yandex_music.sdPlugin/bin/ym-bridge.dylib
```

It should print the same `Good "file" signature for Judd1zzz` line.

---

## Features

### Basic Controls
- **Play/Pause** — pause and resume playback
- **Next/Previous** — switch tracks
- **Like/Dislike** — influence "My Vibe" recommendations
- **Shuffle** — toggle shuffled playback; the button shows the client's state
- **Repeat** — cycle the repeat mode (off → all → one) with a distinct icon per mode
- **Mute** — mute audio without losing volume level
- **Launch Yandex Music** — starts the client without any debug flags; does nothing if it's already connected. On Windows, pressing it (or any plugin button, unless "Allow launching from any plugin button" is off) also restarts a frozen client the plugin hasn't reached for 30 seconds. Handy as the first step of a Multi Action

> **Shuffle/Repeat on "My Vibe":** the plugin sets the mode through the player directly; the context menu is opened only as a fallback. Shuffle is unavailable on "My Vibe" (it's already a stream of different artists), and Repeat there only toggles between off and repeat-one.

### Volume
- **Volume +/-** — adjust volume by 5% per click
- **Volume indicator** — dedicated button showing current level
- **Volume % inside the client** — the plugin draws the exact value above the Yandex Music slider
- **Volume Knob (dials)** — on devices with rotary knobs (Ajazz/Mirabox like AKP05E Pro): rotate to change volume (configurable step, 5% per tick by default), press for Mute or Play/Pause (configurable). Play/Pause, Next/Prev, Like/Dislike, Shuffle, Repeat, Mute, Download and Launch Yandex Music can also be assigned to a knob — pressing it triggers the action

> **Long press:** smooth adjustment on button hold is implemented in code, but may not work on some Stream Deck alternatives. This appears to be a hardware limitation — the press event only fires when the button is released. On original Stream Deck it should work fine, but hasn't been tested yet.

<p align="center"><img src="assets/demo_volume.gif" alt="Volume percentage above the client's slider" width="420"></p>

### Track Info
- **Cover + title + artist** — all on one button
- **Scrolling text** — long titles automatically scroll
- **Press action** — clicking the button toggles play/pause (default), copies "Artist - Track" to clipboard or does nothing — configurable
- **Timecodes on the cover** — optional elapsed / total time overlay on top of the cover art
- **Progress bar** — shows track progress: elapsed time, duration or a bar — several styles to choose from

### Discord Rich Presence
- **Current track in your Discord profile** — "Listening" status with title, artist, cover art and progress
- **Works out of the box** — one toggle in the settings, nothing to create
- Want a custom status name/icon — paste your own `Application ID` (optional)

### Download
- **"Download Track" button** on Stream Deck — saves the current track to a file
- **A button right inside the Yandex Music player** — the plugin adds a download button to the client's player bar, next to like
- Formats: **Lossless (FLAC/M4A)** or **MP3 320**, with tags and cover art; folder and format are configurable

<p align="center"><img src="assets/demo_download.png" alt="Download button in the Yandex Music player bar" width="640"></p>

> Download is available in the GitHub and StreamDock Store builds; the Elgato Marketplace build doesn't have it.

### Video cover
- **Background video in the full-screen player** — when a track has a background video, the client's full-screen player plays it behind the cover. On pause the video stops and hides; tracks without a video keep the regular cover
- **Out of the box:** on; the cover is hidden while the video plays and shows on hover; video brightness 30%, and 50% while the queue or lyrics are open; the backdrop under the player controls is off (once turned on: track accent color, 25% opacity)
- **Your own effects (optional):** sliders for the video's blur, saturation, contrast and black and white, and for the blur, saturation and brightness of the background seen through the backdrop
- Set up in the settings dashboard → Features → Video cover; changes apply without restarting the client

### Page details
Extra information right on the client's pages:
- **Albums** — full release date with a new-release badge, genre, track count and total length
- **Playlists** — row numbers, creation date, track count and total length, a private-playlist badge and "More playlists by the author"
- **Artists** — change in monthly listeners, description, likes, number of albums and tracks, dates of birth and death, real names, genres and countries
- **Out of the box:** everything is on except "Date a track was added" (playlists) and the "AI-generated" badge (artists)
- **Extra requests to Yandex Music** (on by default) — some details take one request per opened page, made through the client itself to Yandex Music, with no third parties and a per-session cache. Turn it off and only what the client already keeps in memory is shown: the genre, album and playlist track counts, the private badge and row numbers; the items that depend on requests are tagged "needs requests". The "AI-generated" badge is not covered by this switch: while the badge is on, its request to `iimuzyka.top` goes out even with extra requests turned off
- Set up in the settings dashboard → Features → Page details, grouped into Playlists, Albums and Artists; changes apply without restarting the client

> ⚠️ **About your data — the "AI-generated" badge.** The badge is off by default. When you turn it on, this plugin sends the ID of every artist you open to the third-party site `iimuzyka.top` — with no account data, cookies or referrer, but the site sees your IP address, as with any request. The "Extra requests to Yandex Music" switch doesn't cover this badge: while the badge is on, its request to `iimuzyka.top` goes out even with extra requests turned off.

### Control from your phone
- **This computer as a device in the Yandex Music app** — with "Control this computer from your phone" on, this computer shows up in the device list of the Yandex Music app on your phone signed in to the same account: play, pause, track switching and volume work from there
- **Off by default** — turn it on in the settings dashboard → Connection. Pressing Play on the computer, including from the deck, takes the music over from the phone; turn the switch off and the computer disappears from the list on the phone right away. If you quit Stream Deck or remove this plugin instead, the mode stays on until the client restarts
- **No requests from the plugin** — the desktop client already has this mode but keeps it off. The plugin only switches it on inside the client and doesn't read your token; the client talks to Yandex Music's own service the same way the mobile app does

> ⚠️ Yandex controls this mode on its side and may close or change it. Then the switch will stop working until this plugin is updated.

### Listening statistics
- **Kept locally** — tracks played, sessions and their length, uptime, downloads and their size, activity by hour and weekday, top artists and tracks, presses per action. View it for 7 or 30 days in the settings dashboard → Analytics → Statistics
- **Where it lives** — since v2.5.0 in the OS data folder, outside the plugin folder: `~/Library/Application Support/com.judd1.yandex_music/stats.json` on macOS, `%APPDATA%\com.judd1.yandex_music\stats.json` on Windows. The GitHub, StreamDock Store and Elgato Marketplace builds share this file
- **Survives updates** — the plugin's self-update, an update from a store and a reinstall don't touch it. On the first v2.5.0 start the old `logs/stats.json` from the plugin folder is copied there, and the old file is renamed to `logs/stats.json.migrated`
- The log (`logs/plugin.log`) and the saved client path stay in the plugin folder

### Auto-update
- **Not in the Elgato Marketplace build** — there updates come from the Marketplace after Elgato reviews each version, which can take several days
- **Update check on startup** — new versions are pulled from GitHub automatically
- **Every update is signature-checked** — the plugin only accepts releases signed with my hardware key ([how it works](#verifying-a-release))
- The feature is new and hasn't been battle-tested yet. If an update doesn't come through — just download the latest release manually, like before: manual installation always works
- Updates are downloaded and written by the plugin itself, so the one-time macOS `xattr` step from the install guide never needs repeating

### Technical Foundation
- **Event-Driven architecture** — instant response, low CPU load when idle
- **Update-resistant** — track state is read from the player's internal store, with DOM selectors (`data-test-id` + fallbacks) as a safety net
- **Native binary** — Rust (`bin/ym-plugin`) with the signed `ym-bridge` library next to it, no Python or Node.js runtime required

---

## What You'll Need

- A **Yandex Plus** subscription — Yandex Music won't play without it
- The **Yandex Music** desktop client (Windows or macOS) from [music.yandex.ru/download](https://music.yandex.ru/download/) — the old Microsoft Store version is not supported
- macOS 12 or newer on Apple Silicon or Intel; Windows 10 or newer (x64)
- A Stream Deck or an alternative (Mirabox/Ajazz) with **v2/v3** software

---

## Installation

### ⚡ Quick install — Elgato Marketplace (Stream Deck)

<p align="left">
  <a href="https://marketplace.elgato.com/product/yandex-music-integration-43741c24-1784-4492-be32-c631d7c55829"><img src="https://docs.elgato.com/img/badges/get-it-on-marketplace--dark.svg" alt="Get it on Elgato Marketplace" height="40"></a>
</p>

Using the official Elgato Stream Deck? The plugin is on the Marketplace — one click and you're done:

1. Open the [plugin page on Elgato Marketplace](https://marketplace.elgato.com/product/yandex-music-integration-43741c24-1784-4492-be32-c631d7c55829).
2. Click **Get** — the Stream Deck app picks it up and installs the plugin automatically.

> How the Marketplace build differs from the GitHub version:
> - no *Download Track* action and no download button inside the client;
> - no *Volume Knob* action, and the other actions go on keys only — none of them can be assigned to a dial;
> - no built-in self-updater: updates come from the Marketplace itself, after Elgato reviews each version, which can take several days, so they may arrive later than on GitHub.
>
> Starting with version 2.5.0.0 on the Marketplace, everything else — the signed bridge, *Video cover*, *Page details*, statistics and Discord — is the same. In the settings dashboard the *Download* section is a "Get the full version" lock, and *Updates* shows a note instead of an update check.

---

### ⚡ Quick install — StreamDock Store (Mirabox/Ajazz)

<p align="left">
  <a href="https://space.key123.vip/product/20260706002752"><img src="assets/badges/get-it-on-streamdock--dark.svg" alt="Get it on StreamDock Store" height="40"></a>
</p>

The plugin is available in the official StreamDock Store — no archives, no Terminal:

1. Sign in (or register for free) at [space.key123.vip](https://space.key123.vip) and open the [plugin page](https://space.key123.vip/product/20260706002752).
2. Click **Open Software** — the StreamDock app opens, downloads and installs the plugin automatically.

> Requires StreamDock **3.10.185.1120 or newer** with your Mirabox Space account signed in inside the app. In the app itself the plugin lives in the **Space** section (not the legacy "Store" tab). On macOS this method needs no quarantine workarounds.
>
> Prefer doing it by hand? Follow steps 1–4 below.

---

### 1. Download the plugin (manual install)

<p align="left">
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases/latest"><img src="https://custom-icon-badges.demolab.com/badge/Download_for_Windows-0078D6?style=for-the-badge&logo=windows11&logoColor=white" alt="Download for Windows"></a>
  <a href="https://github.com/Judd1zzz/yandex-music-streamdeck/releases/latest"><img src="https://img.shields.io/badge/Download_for_macOS-000000?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS"></a>
</p>

Get the latest release from [Releases](https://github.com/Judd1zzz/yandex-music-streamdeck/releases) and unpack the archive. Want to make sure the archive was built by me — a signed `SHA256SUMS` ships right next to it: [how to verify](#verifying-a-release).

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
> Without this, the plugin will fail to start with an "App is damaged" error. The command clears the mark from the whole folder, the bridge library included.
>
> The quarantine mark never comes back on its own — not after a reboot, not after a macOS update. You'll only need this command again if you manually download and unpack a release archive from the browser. Auto-updates are unaffected: the plugin downloads and writes files itself, without the quarantine mark.

</details>

Keep the folder's contents as they are: next to the plugin binary in `bin/` lies the bridge library — `ym-bridge.dll` on Windows, `ym-bridge.dylib` on macOS — with its `.sig` signature. It is the only way the plugin reaches the client, so both files — the library and its `.sig` — have to stay there. Auto-update delivers them along with the plugin.

---

### 3. Launch Yandex Music as usual

The plugin reaches the client through the `ym-bridge` bridge library that ships next to it. The bridge is loaded into the client and opens a private channel that only the plugin can use: no port is opened, and the client needs no launch flags or special shortcuts. Before loading the bridge, the plugin checks its signature — more on that in [Security](#security).

- **Windows:** the plugin attaches to the Yandex Music you already have open — usually without a restart; if the bridge is loaded but doesn't respond, the plugin restarts the client, and if it never loaded, a button press restarts it.
- **macOS:** the bridge can only be loaded as the client starts, so a client opened without it (from the Dock, for example) is restarted once. Takes a few seconds, and the client restores your track and queue on its own.
- **Client not running at all?** Press any plugin button or "Launch Yandex Music" — the plugin starts the client itself.
- **Client installed in a non-standard location?** Set the path once: settings dashboard → Connection → "Client path".

Don't want the plugin to relaunch the client on its own? Turn off "Launch and relaunch the client automatically" in the settings dashboard (Connection tab): the plugin will then relaunch a running client only when you press one of its buttons. A closed client is started only by a button press, whatever this setting.

Still have the special shortcut or launcher app from earlier versions? It's no longer needed — delete it and launch Yandex Music the usual way.

---

### 4. Configure the buttons

1. Open Stream Deck application (Ajazz Dock/Stream Dock)
2. Find the **Yandex Music** category in the actions list on the right
3. Drag the buttons you need onto the panel
4. Click any control button (except Launch Yandex Music) — the settings panel at the bottom will show connection status

If the status shows "Connected" — everything works. If not, the line under the status names the reason; usually it's enough to press any plugin button or "Launch Yandex Music". Common cases are covered in [Problems?](#problems).

---

## Settings

Per button:

| Parameter | Description |
|-----------|-------------|
| **Button style** | Appearance |
| **Displayed elements** | What to show: cover, title, artist, timecodes |
| **Press action** | Track Info click: Play/Pause (default), copy track name or do nothing |
| **Display format** | Progress bar look: timestamps, bar, etc. |

Global — set once in the settings dashboard (the "Open settings dashboard" button in any button's settings), applies everywhere:

| Parameter | Description |
|-----------|-------------|
| **Launch and relaunch the client automatically** | The plugin restarts a running client on its own when it can't reach it (on by default): on macOS a client opened without the bridge is restarted once, on Windows a client whose bridge is loaded but not responding is restarted. When off, the plugin restarts the client only when you press one of its buttons. A closed client is started only by a button press, whatever this setting |
| **Client path** | Only for non-standard installs; empty = auto-detection |
| **Allow launching from any plugin button** | Any plugin button may start the client (on by default); when off, only the Launch Yandex Music action and the button in the settings dashboard do |
| **Discord** | Rich Presence toggle; your own `Application ID` is optional |
| **Download** | Folder and format: Lossless (FLAC/M4A) or MP3 320 |
| **Video cover** | The track's background video in the full-screen player: on/off, hiding the cover, video brightness, brightness with the queue or lyrics open, blur, saturation, contrast and black-and-white sliders, and a backdrop under the player controls — see [Video cover](#video-cover) |
| **Page details** | What to show on album, playlist and artist pages, and "Extra requests to Yandex Music" — see [Page details](#page-details) |
| **Control this computer from your phone** | Off by default. When on, this computer shows up as a device in the Yandex Music app on your phone — see [Control from your phone](#control-from-your-phone) |
| **Language** | Language of the settings dashboard and the button settings: English (default) or Russian |

---

## Ynison mode

Up to v2.4.0 the button settings had a "Control type" switch with an experimental Ynison mode. In the Rust version that mode was a stub, so v2.5.0 removes it: keys that were set to it now control the desktop client like all the others. The Python version in [`python_deprecated/`](python_deprecated/) with its local `api_for_plugin` server is kept for reference — see Technical Details below.

Controlling this computer from your phone is a different feature and needs no separate server — see [Control from your phone](#control-from-your-phone).

---

## Technical Details

<details>
<summary><b>☁️ Ynison API Server</b></summary>

Only the Python version in [`python_deprecated/`](python_deprecated/) uses this server; the Rust plugin doesn't.

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
<summary><b>🖥️ How the Plugin Controls the Client</b></summary>

The plugin controls the Yandex Music client directly — without third-party servers and without a debug port (the only exception is the opt-in "AI-generated" badge, see [Page details](#page-details)).

#### Architecture

```
┌─────────────────────┐                     ┌───────────────────────────────────────┐
│                     │                     │       Yandex Music (Electron)         │
│    Stream Deck      │                     │                                       │
│      Plugin         │   private channel   │   ┌───────────────────────────────┐   │
│      (Rust)         │ ◀─────────────────▶ │   │  ym-bridge (signed library)   │   │
│                     │ mutual verification │   └───────────────┬───────────────┘   │
│                     │                     │   ┌───────────────┴───────────────┐   │
│                     │                     │   │  remote (injected code):      │   │
│                     │                     │   │  player state, commands,      │   │
│                     │                     │   │  download button,             │   │
│                     │                     │   │  video cover, page details,   │   │
│                     │                     │   │  control from the phone       │   │
└─────────────────────┘                     │   └───────────────────────────────┘   │
                                            └───────────────────────────────────────┘
```

#### Data Flow

1. **Connection:**
   - On macOS the `ym-bridge` library is loaded into the client as it starts; on Windows the plugin attaches to a client that is already running
   - The bridge opens a private channel that only the plugin can use — no TCP port is opened
   - Before loading the bridge, the plugin checks its signature; the bridge, in turn, serves only the plugin binary it recognizes

2. **Remote inside the client:**
   - The plugin injects its code into Yandex Music — a single composed remote
   - The remote reads the player state from the app's internal store and reports only what changed
   - Video cover, Page details and control from the phone are parts of the same remote: their settings come from the dashboard over the same channel and apply without restarting the client

3. **Sending commands:**
   - Button presses reach the remote over the same private channel
   - The remote acts through the player directly: Shuffle and Repeat, for example, are set without opening the context menu, and the menu path stays as a fallback

#### Plugin Components

The backend is a Rust workspace in `com.judd1.yandex_music.sdPlugin/src/`:

| Crate | Purpose |
|-------|---------|
| `crates/ym-launch/` | Finding, launching and relaunching the client |
| `crates/ym-remote-controller/` | The channel to the client through the bridge: injects the remote, sends commands, receives state |
| `crates/ym-model/` | State models (`serde`): `MediaState`, `TrackData`, `PlaybackData` |
| `crates/ym-core/` | Actions (buttons), orchestrator, event bus |
| `crates/ym-render/` | Rendering of icons and dynamic buttons |

The `ym-bridge` library (`bin/ym-bridge.dll` / `bin/ym-bridge.dylib`) is a closed component: it ships ready-built and signed next to the plugin binary (see [Security](#security)).

> The code the plugin injects into the client doesn't only read state and press buttons on the plugin's behalf — it also **augments the client's UI**: it adds a download button to the Yandex Music player bar, styled like the native buttons (not in the Elgato Marketplace build), plays the background video in the full-screen player ([Video cover](#video-cover)) and adds details to album, playlist and artist pages ([Page details](#page-details)).

#### Why It's Reliable

- **Reads from the player's internal store** — title, artist, cover, progress and volume come from the app's own state, not from fragile markup
- **DOM fallbacks** — if the store is unavailable, multi-layer selectors (`data-test-id` + alternatives) take over
- **Delta updates** — only changed fields are transmitted, not entire state
- **The button shows the real state** — Play/Pause and Mute follow the client's own updates right away; the volume tile shows the level the player reports
- **Auto-reconnect** — plugin reconnects automatically on connection loss

</details>

## For Developers

The backend is written in **Rust**. Since v2.5.0 the plugin can't be built from this repository alone: the bridge — the `ym-bridge` library, the code it injects into the client and the Windows injector — is closed and ships only ready-built and signed in the release archives (see [Security](#security)), so there are no build instructions here. Audio decoding and remuxing for downloads are pure Rust (Symphonia); MP3 encoding uses a statically linked LAME (libmp3lame, LGPL-3.0) — see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). The previous Python backend is kept for reference in [`python_deprecated/`](python_deprecated/) and is no longer shipped.

---

## Problems?

| Symptom | Solution |
|---------|----------|
| Buttons don't respond | Press any plugin button or "Launch Yandex Music" — the plugin will launch the client itself; on macOS a client opened without the bridge is restarted once. With "Launch and relaunch the client automatically" turned off, the plugin restarts a running client only on a button press |
| "The bridge library isn't next to the plugin" | Antivirus may have quarantined `ym-bridge.dll` / `ym-bridge.dylib`. Restore the file or reinstall the plugin from the release archive, and add the plugin folder to your antivirus exclusions |
| "The bridge library's signature didn't verify" | Reinstall the plugin from the release archive |
| "The client is running as administrator" (Windows) | Untick "Run this program as an administrator" in the client shortcut properties |
| Client in a non-standard folder | Set the path in the settings dashboard → Connection → "Client path" |
| Purple icons | Restart Stream Deck |
| Long press doesn't work | Probably a limitation of your device (see "Volume" section) |
| Discord status doesn't show up | Make sure Discord is running and the Rich Presence toggle is on |
| Update didn't come through | Antivirus may have temporarily locked the files — the plugin retries and will finish the update on the next launch. On Windows, close Yandex Music and restart the Stream Deck app. If your AV flags `ym-plugin.exe` or `ym-bridge.dll`, add the plugin folder to exclusions. Manual installation always works as a fallback |
| The computer doesn't show up on the phone | Turn on "Control this computer from your phone" in the settings dashboard → Connection; the phone has to be signed in to the same Yandex account. If it worked and then stopped, Yandex may have closed this mode — check for a plugin update |
| Control stopped after a Yandex Music update | Update the plugin to the latest version. If that doesn't help, open an [issue](https://github.com/Judd1zzz/yandex-music-streamdeck/issues) and attach `plugin.log` from the plugin's `logs/` folder |

---

## License

Starting with v2.4.0 the source code is licensed under **PolyForm Strict 1.0.0** with a few additional permissions from me — see [LICENSE](LICENSE) for the exact terms. In plain words:

- **You may**: read and audit the published code, modify it for your own personal use, and fork on GitHub to submit pull requests here. Since v2.5.0 the plugin can't be built from this repository alone: the bridge (the `ym-bridge` library, the code it injects into the client and the Windows injector) is closed and ships only ready-built and signed.
- **You may not**: reuse any part of this code in other projects, or distribute the code, builds, or derivative works — free or paid.

Images and badges under `assets/` are not covered by the code license: all rights reserved, with one exception — the unmodified badge may be displayed as a link to this project. Third-party components remain under their own licenses — see [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

Releases up to and including v2.3.0 were published under MIT and remain under it.

---

## Acknowledgments

- **[YandexMusicModClient](https://github.com/TheKing-OfTime/YandexMusicModClient)** — showed which setting of the desktop client opens it to control from the phone; "Control from your phone" in this plugin is my own implementation of that idea.
