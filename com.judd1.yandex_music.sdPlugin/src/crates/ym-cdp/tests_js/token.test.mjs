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

function setup({ fetchImpl, opener }) {
  const dom = new JSDOM(TOKEN_HTML, { runScripts: 'outside-only', pretendToBeVisual: true });
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
    assert.match(window.document.getElementById('status_msg').textContent, /сохранён/);
  });

  test('мёртвый opener: честное сообщение, а не «Токен обновлен»', async () => {
    const window = setup({ fetchImpl: okResponse, opener: null });
    await window.save();
    const msg = window.document.getElementById('status_msg').textContent;
    assert.match(msg, /Не удалось передать токен/);
    assert.ok(!msg.includes('обновлен'), 'нельзя врать об успехе');
    assert.equal(window.document.getElementById('saveBtn').disabled, false);
  });

  test('невалидный токен: сообщение и разблокированная кнопка', async () => {
    const window = setup({ fetchImpl: badResponse, opener: null });
    await window.save();
    assert.match(window.document.getElementById('status_msg').textContent, /Неверный токен/);
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
    assert.match(window.document.getElementById('status_msg').textContent, /не отвечает/);
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
});
