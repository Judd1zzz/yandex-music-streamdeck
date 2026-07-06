import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { JSDOM } from 'jsdom';

const here = dirname(fileURLToPath(import.meta.url));
const STATIC = join(here, '..', '..', '..', '..', 'static');
const PI_HTML = readFileSync(join(STATIC, 'property_inspector.html'), 'utf8');
const PI_JS = readFileSync(join(STATIC, 'js', 'inspector.js'), 'utf8');
const TOKEN_HTML = readFileSync(join(STATIC, 'token.html'), 'utf8');

const PLUGIN_UUID = 'PLUGIN-UUID';
const PI_UUID = 'PI-UUID';

function setup() {
  const dom = new JSDOM(PI_HTML, { runScripts: 'outside-only', pretendToBeVisual: true });
  const { window } = dom;
  const sockets = [];
  class MockWS {
    constructor(url) { this.url = url; this.readyState = 1; this.sent = []; sockets.push(this); }
    send(s) { this.sent.push(JSON.parse(s)); }
    close() {}
  }
  window.WebSocket = MockWS;
  window.alert = () => {};
  window.eval(PI_JS);
  return { window, dom, sockets };
}

function connect(window) {
  const info = JSON.stringify({ pluginUUID: PLUGIN_UUID });
  const actionInfo = JSON.stringify({ action: 'com.judd1.yandex_music.action.like', payload: { settings: {} } });
  window.connectElgatoStreamDeckSocket(123, PI_UUID, 'registerPropertyInspector', info, actionInfo);
}

describe('PI: автономность и надёжность', () => {
  test('никаких внешних ресурсов в разметке (PI и окно токена)', () => {
    const external = /(?:href|src)="https?:\/\//;
    assert.ok(!external.test(PI_HTML), 'property_inspector.html не должен грузить ресурсы из сети');
    assert.ok(!external.test(TOKEN_HTML), 'token.html не должен грузить ресурсы из сети');
  });

  test('защита глобальных настроек не снимается таймером (регрессия потери токена)', async () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    await new Promise((r) => setTimeout(r, 1700));
    window.document.getElementById('discord_app_id_input').value = '111';
    window.updateDiscordAppId();
    const sent = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(sent.length, 0, 'без didReceiveGlobalSettings сохранение должно быть заблокировано всегда');
  });

  test('applyModeToAll: тост вместо alert', () => {
    const { window, sockets } = setup();
    let alerts = 0;
    window.alert = () => { alerts += 1; };
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    window.applyModeToAll();
    const toast = window.document.getElementById('pi_toast');
    assert.ok(!toast.classList.contains('hidden'), 'тост должен показаться');
    assert.equal(toast.textContent, 'Режим применён ко всем кнопкам');
    assert.equal(alerts, 0, 'alert больше не используется');
  });

  test('глобальный селект: клик по download_format пишет в setGlobalSettings без потери соседних полей', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    ws.onmessage({ data: JSON.stringify({ event: 'didReceiveGlobalSettings', payload: { settings: { token: 'T', download_format: 'lossless' } } }) });

    window.document.querySelector('#download_format_items div[data-value="mp3"]').click();

    const sent = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(sent.length, 1, 'клик по опции должен сохранить глобальные настройки');
    assert.equal(sent[0].payload.download_format, 'mp3');
    assert.equal(sent[0].payload.token, 'T', 'соседние глобальные поля не должны теряться');
    assert.equal(
      window.document.getElementById('download_format_selected').textContent,
      'MP3 320',
      'подпись селекта должна обновиться'
    );
  });

  test('updateLocalPort: clamp в диапазон 1-65535, мусор → 9222, уходит числом', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    ws.onmessage({ data: JSON.stringify({ event: 'didReceiveGlobalSettings', payload: { settings: { token: 'T' } } }) });

    const input = window.document.getElementById('local_port_input');
    const cases = [
      ['', 9222],
      ['0', 9222],
      ['99999', 9222],
      ['abc', 9222],
      ['9333', 9333],
    ];
    for (const [raw, expected] of cases) {
      input.value = raw;
      window.updateLocalPort();
      assert.equal(input.value, String(expected), `input для "${raw}"`);
      const last = ws.sent.filter((m) => m.event === 'setGlobalSettings').at(-1);
      assert.strictEqual(last.payload.local_port, expected, `payload для "${raw}"`);
      assert.equal(last.payload.token, 'T', 'остальные глобальные поля не затёрты');
    }
  });
});

describe('Ynison: предупреждение при переключении режима', () => {
  function openPi() {
    const env = setup();
    connect(env.window);
    const ws = env.sockets[0];
    ws.onopen();
    return { ...env, ws };
  }
  const modal = (window) => window.document.getElementById('ynison_modal');
  const option = (window, value) =>
    window.document.querySelector(`#control_mode_items div[data-value="${value}"]`);
  const modeSaves = (ws) =>
    ws.sent.filter((m) => m.event === 'setSettings' && m.payload.control_mode === 'ynison');

  test('клик по ynison открывает модалку и НЕ применяет режим', () => {
    const { window, ws } = openPi();
    const label = window.document.getElementById('control_mode_selected');
    const before = label.textContent;
    option(window, 'ynison').click();
    assert.ok(!modal(window).classList.contains('hidden'), 'модалка должна показаться');
    assert.equal(modeSaves(ws).length, 0, 'setSettings с ynison не должен уходить до подтверждения');
    assert.notEqual(window.settings.control_mode, 'ynison');
    assert.equal(label.textContent, before, 'текст селекта не должен меняться до подтверждения');
  });

  test('«Отмена» закрывает модалку, режим остаётся прежним', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_cancel').click();
    assert.ok(modal(window).classList.contains('hidden'));
    assert.equal(modeSaves(ws).length, 0);
    assert.notEqual(window.settings.control_mode, 'ynison');
  });

  test('«Я осознаю» применяет ynison: сохранение + переключение UI', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_confirm').click();
    assert.ok(modal(window).classList.contains('hidden'));
    assert.equal(modeSaves(ws).length, 1);
    assert.equal(window.settings.control_mode, 'ynison');
    assert.ok(!window.document.getElementById('token_settings_block').classList.contains('hidden'));
    assert.ok(window.document.getElementById('local_settings_group').classList.contains('hidden'));
  });

  test('переключение обратно на local — без модалки, применяется сразу', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_confirm').click();
    option(window, 'local').click();
    assert.ok(modal(window).classList.contains('hidden'));
    assert.equal(window.settings.control_mode, 'local');
    const localSaves = ws.sent.filter(
      (m) => m.event === 'setSettings' && m.payload.control_mode === 'local'
    );
    assert.ok(localSaves.length >= 1);
  });

  test('клик по ynison, когда режим уже ynison — без модалки', () => {
    const { window } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_confirm').click();
    option(window, 'ynison').click();
    assert.ok(modal(window).classList.contains('hidden'), 'повторный выбор не должен спрашивать');
  });

  test('Esc закрывает модалку без применения, фокус — на «Отмена»', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    assert.equal(
      window.document.activeElement,
      window.document.getElementById('ynison_cancel'),
      'фокус должен встать на «Отмена»'
    );
    window.document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape' }));
    assert.ok(modal(window).classList.contains('hidden'), 'Esc должен закрыть модалку');
    assert.equal(modeSaves(ws).length, 0);
    assert.notEqual(window.settings.control_mode, 'ynison');
    window.document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape' }));
    assert.ok(modal(window).classList.contains('hidden'), 'повторный Esc безвреден');
  });

  test('после отмены модалку можно открыть снова', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_cancel').click();
    option(window, 'ynison').click();
    assert.ok(!modal(window).classList.contains('hidden'));
    window.document.getElementById('ynison_confirm').click();
    assert.equal(modeSaves(ws).length, 1);
    assert.equal(window.settings.control_mode, 'ynison');
  });
});

describe('Property Inspector: global settings (StreamDock context)', () => {
  test('getGlobalSettings шлётся с context: uuid (PI), а НЕ pluginUUID', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    const ggs = ws.sent.find((m) => m.event === 'getGlobalSettings');
    assert.ok(ggs, 'должен быть getGlobalSettings');
    assert.equal(ggs.context, PI_UUID);
    assert.notEqual(ggs.context, PLUGIN_UUID);
  });

  test('saveGlobalSettings заблокирован до didReceiveGlobalSettings (нет частичного payload)', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    window.document.getElementById('discord_app_id_input').value = '111';
    window.updateDiscordAppId();
    const sent = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(sent.length, 0, 'guard globalsReady должен блокировать сохранение до сидинга');
  });

  test('после didReceiveGlobalSettings: setGlobalSettings шлёт context: uuid и ПОЛНЫЙ объект', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    ws.onmessage({ data: JSON.stringify({ event: 'didReceiveGlobalSettings', payload: { settings: { discord_app_id: '111', download_format: 'mp3' } } }) });

    window.document.getElementById('chk_discord_rpc').checked = true;
    window.updateDiscordEnabled();

    const sent = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(sent.length, 1);
    const msg = sent[0];
    assert.equal(msg.context, PI_UUID);
    assert.notEqual(msg.context, PLUGIN_UUID);
    // ничего не затёрто — поля seed'а сохранены вместе с новым
    assert.equal(msg.payload.discord_app_id, '111');
    assert.equal(msg.payload.download_format, 'mp3');
    assert.equal(msg.payload.discord_rpc_enabled, true);
  });
});

describe('PI: причина проблемы автозапуска', () => {
  function sendStatus(ws, payload) {
    ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload }) });
  }

  test('LocalStatus c reason показывает строку-подсказку', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendStatus(ws, { event: 'LocalStatus', status: 'disconnected', reason: 'Клиент запущен от имени администратора' });
    const el = window.document.getElementById('local_status_reason');
    assert.equal(el.textContent, 'Клиент запущен от имени администратора');
    assert.ok(!el.classList.contains('hidden'), 'строка причины должна быть видима');
  });

  test('LocalStatus без reason прячет строку', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendStatus(ws, { event: 'LocalStatus', status: 'disconnected', reason: 'Порт занят' });
    sendStatus(ws, { event: 'LocalStatus', status: 'disconnected' });
    const el = window.document.getElementById('local_status_reason');
    assert.equal(el.textContent, '');
    assert.ok(el.classList.contains('hidden'));
  });

  test('при connected причина скрыта, даже если пришла в payload', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendStatus(ws, { event: 'LocalStatus', status: 'connected', reason: 'устаревшая причина' });
    const el = window.document.getElementById('local_status_reason');
    assert.ok(el.classList.contains('hidden'));
  });

  test('TokenStatus (режим Ynison) скрывает local-причину', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendStatus(ws, { event: 'LocalStatus', status: 'disconnected', reason: 'Клиент не найден' });
    sendStatus(ws, { event: 'TokenStatus', status: 'missing' });
    const el = window.document.getElementById('local_status_reason');
    assert.ok(el.classList.contains('hidden'));
  });
});

describe('PI: уведомление об установленном обновлении', () => {
  function sendNotice(ws, payload) {
    ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload }) });
  }

  test('UpdateNotice показывает русский текст с версией и ссылку', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    const el = window.document.getElementById('update_notice');
    assert.ok(el.classList.contains('hidden'), 'до уведомления блок скрыт');

    sendNotice(ws, { event: 'UpdateNotice', version: '2.1.3' });
    assert.ok(!el.classList.contains('hidden'));
    assert.match(el.textContent, /2\.1\.3/);
    assert.match(el.textContent, /перезапустите/i);
    assert.ok(el.querySelector('.pi-update-link'), 'есть ссылка «Что нового»');
  });

  test('UpdateNotice без версии оставляет блок скрытым', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendNotice(ws, { event: 'UpdateNotice' });
    const el = window.document.getElementById('update_notice');
    assert.ok(el.classList.contains('hidden'));
  });

  test('клик по «Что нового» шлёт openUrl на страницу релиза', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendNotice(ws, { event: 'UpdateNotice', version: '2.1.3' });
    window.document.querySelector('#update_notice .pi-update-link').click();
    const open = ws.sent.find((m) => m.event === 'openUrl');
    assert.ok(open, 'должно уйти событие openUrl');
    assert.equal(open.payload.url, 'https://github.com/Judd1zzz/yandex-music-streamdeck/releases/tag/v2.1.3');
  });

  test('повторный UpdateNotice идемпотентен (одна ссылка, свежая версия)', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    sendNotice(ws, { event: 'UpdateNotice', version: '2.1.3' });
    sendNotice(ws, { event: 'UpdateNotice', version: '2.1.4' });
    const el = window.document.getElementById('update_notice');
    assert.match(el.textContent, /2\.1\.4/);
    assert.equal(el.querySelectorAll('.pi-update-link').length, 1);
  });
});

describe('PI: живая валидация пути клиента', () => {
  function prepared() {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    return { window, ws };
  }
  function deliver(ws, payload) {
    ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload }) });
  }

  test('изменение пути шлёт check_client_path с адресом ответа', () => {
    const { window, ws } = prepared();
    const input = window.document.getElementById('client_path_input');
    input.value = 'C:\\Custom\\YandexMusic';
    window.updateClientPath();

    const sent = ws.sent.filter((m) => m.event === 'sendToPlugin' && m.payload.event === 'check_client_path');
    assert.equal(sent.length, 1);
    assert.equal(sent[0].payload.path, 'C:\\Custom\\YandexMusic');
    assert.equal(sent[0].payload.reply_action, 'com.judd1.yandex_music.action.like');
  });

  test('пустой путь не шлёт запрос и прячет строку', () => {
    const { window, ws } = prepared();
    const input = window.document.getElementById('client_path_input');
    input.value = '   ';
    window.updateClientPath();

    const sent = ws.sent.filter((m) => m.event === 'sendToPlugin' && m.payload.event === 'check_client_path');
    assert.equal(sent.length, 0);
    assert.ok(window.document.getElementById('client_path_check').classList.contains('hidden'));
  });

  test('вердикты рендерятся своими текстами и тонами', () => {
    const { window, ws } = prepared();
    const el = window.document.getElementById('client_path_check');

    deliver(ws, { event: 'ClientPathCheck', verdict: 'ok', resolved: null, expected: 'Яндекс Музыка.exe' });
    assert.equal(el.textContent, '✓ Клиент найден');
    assert.ok(el.className.includes('pi-path-ok'));
    assert.ok(!el.className.includes('hidden'));

    deliver(ws, {
      event: 'ClientPathCheck',
      verdict: 'ok_dir',
      resolved: 'C:\\Custom\\YandexMusic\\Яндекс Музыка.exe',
      expected: 'Яндекс Музыка.exe',
    });
    assert.ok(el.textContent.includes('C:\\Custom\\YandexMusic\\Яндекс Музыка.exe'));
    assert.ok(el.className.includes('pi-path-warn'));

    deliver(ws, { event: 'ClientPathCheck', verdict: 'missing', resolved: null, expected: 'Яндекс Музыка.exe' });
    assert.equal(el.textContent, 'Путь не существует');
    assert.ok(el.className.includes('pi-path-err'));

    deliver(ws, { event: 'ClientPathCheck', verdict: 'dir_without_client', resolved: null, expected: 'Яндекс Музыка.app' });
    assert.ok(el.textContent.includes('Яндекс Музыка.app'));
    assert.ok(el.className.includes('pi-path-err'));
  });

  test('неизвестный вердикт прячет строку', () => {
    const { window, ws } = prepared();
    const el = window.document.getElementById('client_path_check');
    deliver(ws, { event: 'ClientPathCheck', verdict: 'ok', resolved: null, expected: 'x' });
    assert.ok(!el.className.includes('hidden'));
    deliver(ws, { event: 'ClientPathCheck', verdict: 'что-то-новое', resolved: null, expected: 'x' });
    assert.ok(el.className.includes('hidden'));
    assert.equal(el.textContent, '');
  });
});
