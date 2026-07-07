# Privacy Policy

**Yandex Music Integration** (Stream Deck / StreamDock plugin)

Last updated: 2026-07-07

## What the plugin does with your data

The plugin does **not** collect, store, or transmit any personal data, analytics, or telemetry. There are no trackers and no third-party analytics services.

## What stays on your computer

- Plugin settings (button styles, control mode, download preferences where available) are stored locally by the Stream Deck / StreamDock host application.
- An optional Yandex Music OAuth token (used only for the experimental cloud mode and track metadata requests, in builds where those features are present) is stored locally in the host application's global settings and is never sent anywhere except Yandex Music API endpoints.

## Network connections the plugin makes

- `api.music.yandex.net` — Yandex Music API requests made on your behalf (track metadata; downloads in builds that include the download feature).
- `avatars.yandex.net` — album cover images for Discord Rich Presence.
- `127.0.0.1` (localhost) — communication with the Yandex Music desktop app via Chrome DevTools Protocol and with the Stream Deck host application. Never leaves your computer.
- Discord Rich Presence uses a local IPC connection to your running Discord client. Never leaves your computer.
- GitHub Releases API (`api.github.com`) — **only in builds distributed via GitHub/StreamDock** to check for plugin updates. The Elgato Marketplace build does not contain a self-updater; updates are delivered by the Marketplace itself.

## Your account

Sign-in happens exclusively inside the official Yandex Music application. The plugin never sees or asks for your password.

## Contact

Questions or concerns: https://github.com/Judd1zzz/yandex-music-streamdeck/issues
