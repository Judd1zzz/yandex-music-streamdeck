import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { JSDOM } from 'jsdom';

const here = dirname(fileURLToPath(import.meta.url));
const STATIC = join(here, '..', '..', '..', '..', 'static');
const TOKEN_HTML = readFileSync(join(STATIC, 'token.html'), 'utf8');
const TOKEN_JS = readFileSync(join(STATIC, 'js', 'token.js'), 'utf8');

function setup({ fetchImpl, opener, url }) {
  const dom = new JSDOM(TOKEN_HTML, {
    runScripts: 'outside-only',
    pretendToBeVisual: true,
    url: url || 'file:///token.html',
  });
  const { window } = dom;
  window.fetch = fetchImpl;
  window.close = () => {};
  Object.defineProperty(window, 'opener', { value: opener, configurable: true });
  window.eval(TOKEN_JS);
  window.document.getElementById('token').value = 'TOKEN-123';
  return window;
}

const okResponse = async () => ({ json: async () => ({ valid: true }) });
const badResponse = async () => ({ json: async () => ({ valid: false }) });

describe('Окно токена (token.js)', () => {
  test('валидный токен передаётся в PI через opener.updateToken', async () => {
    let received = null;
    const opener = { updateToken: (t) => { received = t; } };
    const window = setup({ fetchImpl: okResponse, opener });
    await window.save();
    assert.equal(received, 'TOKEN-123');
    assert.match(window.document.getElementById('status_msg').textContent, /saved/);
  });

  test('мёртвый opener: честное сообщение, а не рапорт об успехе', async () => {
    const window = setup({ fetchImpl: okResponse, opener: null });
    await window.save();
    const msg = window.document.getElementById('status_msg').textContent;
    assert.match(msg, /Could not hand the token over/);
    assert.ok(!/saved/i.test(msg), 'нельзя врать об успехе');
    assert.equal(window.document.getElementById('saveBtn').disabled, false);
  });

  test('невалидный токен: сообщение и разблокированная кнопка', async () => {
    const window = setup({ fetchImpl: badResponse, opener: null });
    await window.save();
    assert.match(window.document.getElementById('status_msg').textContent, /Invalid token/);
    assert.equal(window.document.getElementById('saveBtn').disabled, false);
  });

  test('API-сервер не запущен: внятная подсказка про api_for_plugin', async () => {
    const window = setup({
      fetchImpl: async () => { throw new TypeError('fetch failed'); },
      opener: null,
    });
    await window.save();
    assert.match(window.document.getElementById('status_msg').textContent, /api_for_plugin/);
    assert.equal(window.document.getElementById('saveBtn').disabled, false);
  });

  test('таймаут запроса: сообщение про не отвечающий сервер', async () => {
    const window = setup({
      fetchImpl: async () => {
        const e = new Error('aborted');
        e.name = 'AbortError';
        throw e;
      },
      opener: null,
    });
    await window.save();
    assert.match(window.document.getElementById('status_msg').textContent, /not responding/);
    assert.equal(window.document.getElementById('saveBtn').disabled, false);
  });

  test('пустой ввод — запроса нет', async () => {
    let called = 0;
    const window = setup({
      fetchImpl: async () => { called += 1; return okResponse(); },
      opener: null,
    });
    window.document.getElementById('token').value = '   ';
    await window.save();
    assert.equal(called, 0);
  });

  test('без ?lang интерфейс английский', async () => {
    const window = setup({ fetchImpl: badResponse, opener: null });
    const doc = window.document;
    assert.equal(doc.documentElement.lang, 'en');
    assert.equal(doc.querySelector('h2').textContent, 'Authentication');
    assert.ok(!/[А-Яа-яЁё]/.test(doc.getElementById('token').getAttribute('placeholder')));
  });

  test('?lang=ru: статика и статусы на русском', async () => {
    const window = setup({ fetchImpl: badResponse, opener: null, url: 'file:///token.html?lang=ru' });
    const doc = window.document;
    assert.equal(doc.documentElement.lang, 'ru');
    assert.equal(doc.querySelector('h2').textContent, 'Аутентификация');
    assert.equal(doc.getElementById('token').getAttribute('placeholder'), 'Вставьте токен сюда...');
    assert.equal(doc.getElementById('saveBtn').textContent, 'Обновить токен');
    await window.save();
    assert.match(doc.getElementById('status_msg').textContent, /Неверный токен/);
  });

  test('словари token-окна покрывают одинаковый набор ключей', () => {
    const window = setup({ fetchImpl: badResponse, opener: null });
    assert.deepEqual(Object.keys(window.TOKEN_I18N.en).sort(), Object.keys(window.TOKEN_I18N.ru).sort());
  });
});
