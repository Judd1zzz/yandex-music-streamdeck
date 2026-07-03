# Python-бэкенд (УСТАРЕЛО)

Это **прежняя Python-реализация** бэкенда плагина `com.judd1.yandex_music.sdPlugin`.
Она **заменена Rust-портом** (`../rust/`), который теперь и поставляется (манифест
плагина запускает скомпилированный бинарь `bin/ym-plugin`, а не `run.sh`/`main.py`).

Оставлена для **сверки и отката**, не входит в поставляемый пакет плагина.

## Что здесь

- `main.py`, `run.sh`, `requirements.txt` — entrypoint и запуск бэкенда.
- `src/` — `actions/` и `core/` (CDP-контроллер, рендереры, схемы, роутинг и т.д.).
- `tests/` — Python-тесты (`unittest`).
- `tools/build.py` — упаковщик PyInstaller.

## Чего здесь НЕТ

- `injected_api.js` — он живой (вшивается в Rust-бинарь), переехал в
  `../rust/crates/ym-cdp/assets/injected_api.js`; его Node-тесты — в
  `../rust/crates/ym-cdp/tests_js/`. Поэтому `src/core/cdp.py` тут на него уже не
  указывает — для запуска этой Python-версии скрипт нужно вернуть в
  `src/core/scripts/injected_api.js`.

## Откат на Python (если понадобится)

Вернуть в `../com.judd1.yandex_music.sdPlugin/manifest.json`:

```json
"CodePathMac": "run.sh",
"CodePathWin": "run.bat"
```

и положить эту папку обратно в `com.judd1.yandex_music.sdPlugin/` (плюс вернуть
`injected_api.js` в `src/core/scripts/`).

## Ynison

Облачный режим Ynison обслуживает отдельный сервис `../api_for_plugin/` (Python
FastAPI) — он **не устарел**: Rust-крейт `ym-ynison` станет его клиентом в v1.1.
