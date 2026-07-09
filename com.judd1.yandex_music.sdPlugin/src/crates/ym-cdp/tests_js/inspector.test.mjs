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

function connect(window, action = 'com.judd1.yandex_music.action.like') {
  const info = JSON.stringify({ pluginUUID: PLUGIN_UUID });
  const actionInfo = JSON.stringify({ action, payload: { settings: {} } });
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
    assert.equal(toast.textContent, 'Mode applied to all buttons');
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

  test('ссылка про api_for_plugin открывает README-гайд, модалка не закрывается', () => {
    const { window, ws } = openPi();
    option(window, 'ynison').click();
    window.document.getElementById('ynison_repo_link').click();
    const open = ws.sent.find((m) => m.event === 'openUrl');
    assert.ok(open, 'должно уйти событие openUrl');
    assert.equal(open.payload.url, 'https://github.com/Judd1zzz/yandex-music-streamdeck#ynison-mode-experimental');
    assert.ok(!modal(window).classList.contains('hidden'), 'модалка остаётся открытой');
    assert.equal(modeSaves(ws).length, 0, 'клик по ссылке не применяет режим');
  });

  test('ссылка про api_for_plugin следует языку: русский текст и README_RU', () => {
    const { window, ws } = openPi();
    ws.onmessage({ data: JSON.stringify({ event: 'didReceiveGlobalSettings', payload: { settings: { pi_language: 'ru' } } }) });
    const link = window.document.getElementById('ynison_repo_link');
    assert.equal(link.textContent, 'Как запустить api_for_plugin — инструкция на GitHub');
    link.click();
    const open = ws.sent.find((m) => m.event === 'openUrl');
    assert.ok(open.payload.url.includes('README_RU.md#'), 'русский язык ведёт на README_RU');
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

  test('UpdateNotice показывает текст с версией и ссылку', () => {
    const { window, sockets } = setup();
    connect(window);
    const ws = sockets[0];
    ws.onopen();
    const el = window.document.getElementById('update_notice');
    assert.ok(el.classList.contains('hidden'), 'до уведомления блок скрыт');

    sendNotice(ws, { event: 'UpdateNotice', version: '2.1.3' });
    assert.ok(!el.classList.contains('hidden'));
    assert.match(el.textContent, /2\.1\.3/);
    assert.match(el.textContent, /restart Stream Deck/i);
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
    assert.equal(el.textContent, '✓ Client found');
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
    assert.equal(el.textContent, 'Path does not exist');
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

describe('PI: маршрутизация UUID двух схем (StreamDock underscore / Elgato dash)', () => {
  test('legacy underscore-uuid: секция открывается, download-блок остаётся видимым', () => {
    const { window, sockets } = setup();
    connect(window, 'com.judd1.yandex_music.action.like');
    sockets[0].onopen();
    const doc = window.document;
    assert.ok(!doc.getElementById('like_settings').classList.contains('hidden'));
    assert.ok(!doc.getElementById('download_global_block').classList.contains('hidden'));
  });

  test('dash-uuid: секция открывается по нормализации, download-блок скрыт', () => {
    const { window, sockets } = setup();
    connect(window, 'com.judd1.yandex-music.action.like');
    sockets[0].onopen();
    const doc = window.document;
    assert.ok(!doc.getElementById('like_settings').classList.contains('hidden'));
    assert.ok(doc.getElementById('download_global_block').classList.contains('hidden'));
  });

  test('dash-uuid volume-display: открывает volume-секцию', () => {
    const { window, sockets } = setup();
    connect(window, 'com.judd1.yandex-music.action.volume-display');
    sockets[0].onopen();
    assert.ok(!window.document.getElementById('volume_settings').classList.contains('hidden'));
  });

  test('legacy volume_knob: обе секции открываются как раньше', () => {
    const { window, sockets } = setup();
    connect(window, 'com.judd1.yandex_music.action.volume_knob');
    sockets[0].onopen();
    const doc = window.document;
    assert.ok(!doc.getElementById('knob_settings').classList.contains('hidden'));
    assert.ok(!doc.getElementById('volume_settings').classList.contains('hidden'));
  });
});

describe('PI: локализация (EN/RU)', () => {
  function setLocale(window, locale) {
    Object.defineProperty(window.navigator, 'language', { value: locale, configurable: true });
    Object.defineProperty(window.navigator, 'languages', { value: [locale], configurable: true });
  }
  function opened(globals = {}, locale = 'en-US') {
    const env = setup();
    setLocale(env.window, locale);
    connect(env.window);
    const ws = env.sockets[0];
    ws.onopen();
    ws.onmessage({ data: JSON.stringify({ event: 'didReceiveGlobalSettings', payload: { settings: globals } }) });
    return { ...env, ws };
  }
  const cyrillic = /[А-Яа-яЁё]/;

  test('каждый data-i18n ключ разметки существует в обоих словарях', () => {
    const { window } = setup();
    const doc = window.document;
    const keys = new Set();
    doc.querySelectorAll('[data-i18n]').forEach((el) => keys.add(el.getAttribute('data-i18n')));
    doc.querySelectorAll('[data-i18n-placeholder]').forEach((el) => keys.add(el.getAttribute('data-i18n-placeholder')));
    assert.ok(keys.size >= 30, 'разметка должна быть размечена для перевода');
    for (const key of keys) {
      assert.notEqual(window.I18N.en[key], undefined, `en: нет ключа ${key}`);
      assert.notEqual(window.I18N.ru[key], undefined, `ru: нет ключа ${key}`);
    }
  });

  test('словари en и ru покрывают одинаковый набор ключей', () => {
    const { window } = setup();
    assert.deepEqual(Object.keys(window.I18N.en).sort(), Object.keys(window.I18N.ru).sort());
  });

  test('дефолт en: после сидинга без pi_language интерфейс английский, кириллицы нет', () => {
    const { window } = opened({}, 'en-US');
    const doc = window.document;
    assert.equal(doc.documentElement.lang, 'en');
    assert.equal(doc.getElementById('mode_description').textContent, 'Controls the client on this computer');
    doc.querySelectorAll('[data-i18n]').forEach((el) => {
      assert.ok(!cyrillic.test(el.textContent), `узел ${el.getAttribute('data-i18n')} должен быть английским`);
    });
    assert.ok(!cyrillic.test(doc.getElementById('client_path_input').getAttribute('placeholder')));
    assert.ok(doc.getElementById('lang_offer').classList.contains('hidden'), 'плашка не показывается на не-русской системе');
  });

  test('pi_language=ru из globalSettings русифицирует статические и динамические тексты', () => {
    const { window } = opened({ pi_language: 'ru' });
    const doc = window.document;
    assert.equal(doc.documentElement.lang, 'ru');
    const label = doc.querySelector('[data-i18n="control_mode_label"]');
    assert.equal(label.textContent, 'Тип управления');
    assert.equal(doc.getElementById('mode_description').textContent, 'Управление клиентом на этом компьютере');
    assert.equal(doc.getElementById('client_path_input').getAttribute('placeholder'), 'Пусто — автоопределение');
    assert.ok(doc.getElementById('lang_offer').classList.contains('hidden'), 'выбор сделан — плашки нет');
  });

  test('русская система без выбора: плашка видна, «Переключить» сохраняет ru и русифицирует', () => {
    const { window, ws } = opened({ token: 'T' }, 'ru-RU');
    const doc = window.document;
    const offer = doc.getElementById('lang_offer');
    assert.ok(!offer.classList.contains('hidden'), 'плашка должна показаться');
    assert.match(offer.textContent, /на русском/);

    doc.getElementById('lang_offer_yes').click();
    const saves = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(saves.length, 1);
    assert.equal(saves[0].payload.pi_language, 'ru');
    assert.equal(saves[0].payload.token, 'T', 'соседние глобальные поля не теряются');
    assert.ok(offer.classList.contains('hidden'));
    assert.equal(doc.querySelector('[data-i18n="control_mode_label"]').textContent, 'Тип управления');
  });

  test('русская система: «Оставить English» сохраняет en и прячет плашку навсегда', () => {
    const { window, ws } = opened({}, 'ru-RU');
    const doc = window.document;
    doc.getElementById('lang_offer_no').click();
    const saves = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(saves.length, 1);
    assert.equal(saves[0].payload.pi_language, 'en');
    assert.ok(doc.getElementById('lang_offer').classList.contains('hidden'));
    assert.equal(doc.querySelector('[data-i18n="control_mode_label"]').textContent, 'Control mode');
  });

  test('русская система с сохранённым выбором: плашка не показывается', () => {
    const { window } = opened({ pi_language: 'en' }, 'ru-RU');
    assert.ok(window.document.getElementById('lang_offer').classList.contains('hidden'));
  });

  test('переключатель Language: выбор ru пишет pi_language и переводит на лету', () => {
    const { window, ws } = opened({ download_format: 'mp3' });
    const doc = window.document;
    doc.querySelector('#pi_language_items div[data-value="ru"]').click();
    const saves = ws.sent.filter((m) => m.event === 'setGlobalSettings');
    assert.equal(saves.length, 1);
    assert.equal(saves[0].payload.pi_language, 'ru');
    assert.equal(saves[0].payload.download_format, 'mp3');
    assert.equal(doc.querySelector('[data-i18n="settings_note"]').textContent, 'Настройки сохраняются сразу');
    assert.equal(doc.getElementById('pi_language_selected').textContent, 'Русский');
  });

  test('смена языка перерисовывает уже показанные статус и update-notice', () => {
    const { window, ws } = opened({});
    const doc = window.document;
    ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload: { event: 'LocalStatus', status: 'connected' } }) });
    ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload: { event: 'UpdateNotice', version: '2.3.0' } }) });
    assert.equal(doc.getElementById('local_status_indicator').textContent, 'CONNECTED');

    doc.querySelector('#pi_language_items div[data-value="ru"]').click();
    assert.equal(doc.getElementById('local_status_indicator').textContent, 'ПОДКЛЮЧЕНО');
    assert.match(doc.getElementById('update_notice').textContent, /перезапустите Stream Deck/);
    assert.equal(doc.querySelectorAll('#update_notice .pi-update-link').length, 1, 'ссылка не дублируется при перерисовке');
  });

  test('reason-коды плагина переводятся, неизвестный reason показывается как есть', () => {
    const { window, ws } = opened({});
    const doc = window.document;
    const el = doc.getElementById('local_status_reason');
    const send = (reason) =>
      ws.onmessage({ data: JSON.stringify({ event: 'sendToPropertyInspector', payload: { event: 'LocalStatus', status: 'disconnected', reason } }) });

    send('client_not_found');
    assert.match(el.textContent, /Yandex Music client not found/);

    doc.querySelector('#pi_language_items div[data-value="ru"]').click();
    assert.match(el.textContent, /Клиент Яндекс Музыки не найден/);

    send('port_busy');
    assert.match(el.textContent, /Порт 9222 занят/);

    send('произвольная строка от будущей версии');
    assert.equal(el.textContent, 'произвольная строка от будущей версии');
  });

  test('окно токена открывается с текущим языком в query', () => {
    const { window } = opened({ pi_language: 'ru' });
    const urls = [];
    window.open = (url) => { urls.push(url); return null; };
    window.openTokenPopup();
    assert.equal(urls.length, 1);
    assert.ok(urls[0].endsWith('token.html?lang=ru'));
  });
});
