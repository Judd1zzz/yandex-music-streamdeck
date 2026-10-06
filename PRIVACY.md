# Privacy Policy

**Yandex Music Integration** (Stream Deck / StreamDock plugin)

Last updated: October 6, 2026

## What the plugin does with your data

The plugin does **not** send any analytics or telemetry to me or to any analytics service; the only third parties involved are the optional features listed below. There are no trackers and no third-party analytics services. The plugin keeps a few files on your computer and makes only the network connections listed below. The one connection to a third-party site happens only if you turn on the optional "AI-generated" badge, which is off by default.

## What stays on your computer

- Plugin settings (button styles, download preferences where available, video cover and page details preferences, and whether control from your phone is on) are stored locally by the Stream Deck / StreamDock host application.
- Listening statistics (tracks played, sessions, uptime, downloads, activity by hour and day, top artists and tracks, button press counters) are stored in `stats.json` in the system's app data folder: `~/Library/Application Support/com.judd1.yandex_music/` on macOS, `%APPDATA%\com.judd1.yandex_music\` on Windows. Versions before 2.5.0 kept this file in `logs/` inside the plugin folder; on its first start 2.5.0 copies it to the new location once and renames the old file to `logs/stats.json.migrated`. If both builds (GitHub/StreamDock and Elgato) are installed, the statistics of the one that starts first are moved; the other build's old file stays in its `logs/` folder. The statistics are never sent anywhere.
- The plugin log (`logs/plugin.log`) and the saved path to the Yandex Music client (`.ym_client_path`) stay in the plugin folder.
- The plugin does not store a Yandex Music token. In builds that include the download feature, when you download a track, the plugin takes the token from the running Yandex Music client's session; it is used only for that download's requests to the Yandex Music API, and the plugin does not save it. A token entered in earlier versions is erased from the host application's global settings on the first start of 2.5.0. The Elgato Marketplace build has no downloads and does not use a token.

## Network connections the plugin makes

- `api.music.yandex.net` — Yandex Music API requests made on your behalf (track metadata and downloads in builds that include the download feature). In builds with downloads, the audio file itself is fetched from the Yandex Music storage address that this API returns.
- `avatars.yandex.net` — cover images for the Track Info key and, in builds with downloads, for the downloaded file's tags. For Discord Rich Presence the plugin passes the cover address to Discord, which loads it itself.
- Requests made by the Yandex Music client itself for the "Page details" feature — only while "Extra requests to Yandex Music" is on (it is on by default): when you open an album, playlist or artist page, the plugin asks the client to load that page's details — one request per page, sent by the client to Yandex Music with the client's own resources, cached for the session, with no third parties involved. Turn the option off and only what the client already keeps in memory is shown.
- `iimuzyka.top` — a third-party site, contacted **only if you turn on the "AI-generated" badge** in Page details (off by default). Then, for every artist page you open, the client sends that artist's ID (the number from the page address) to `https://iimuzyka.top/`. The request carries no cookies, no account data, and no referrer, but the site sees your IP address, as with any request.
- The "Video cover" feature plays the background video that Yandex Music provides for the current track; the client loads it from the address in the track's data, and the plugin adds no data of its own to that request.
- "Control this computer from your phone" (off by default) — when you turn it on, the plugin switches on the Yandex Music client's own mode that lists this computer as a device in the Yandex Music app on your phone. The plugin makes no requests of its own for this and does not read your token: the client exchanges playback state and commands with Yandex Music's own service, the same way the mobile app does. When you turn it off, the client reloads its settings from Yandex Music and reconnects. If you quit Stream Deck or remove the plugin instead, the mode stays on until the client restarts.
- Communication with the Yandex Music desktop app goes through the plugin's signed bridge library (`ym-bridge`), which is loaded into the client and opens a private local channel between the plugin and the client. No debug port is opened. Never leaves your computer.
- `127.0.0.1` (localhost) — communication with the Stream Deck host application. Never leaves your computer.
- Discord Rich Presence (off by default) talks to your running Discord client over local IPC; Discord then shows the track title, artist and cover on your profile — that is the feature.
- GitHub (`api.github.com` and the release downloads it points to) — **only in builds distributed via GitHub/StreamDock**, to check for plugin updates and download them. The Elgato Marketplace build does not contain a self-updater; updates are delivered by the Marketplace itself.

## Your account

Sign-in happens exclusively inside the official Yandex Music application. The plugin never sees or asks for your password.

## Contact

Questions or concerns: https://github.com/Judd1zzz/yandex-music-streamdeck/issues
