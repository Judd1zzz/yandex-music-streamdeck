import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { JSDOM } from 'jsdom';

const here = dirname(fileURLToPath(import.meta.url));
const SRC_PATH = join(here, '..', '..', 'assets', 'injected_api.js');
const SRC = readFileSync(SRC_PATH, 'utf8');

export function mount(html, opts = {}) {
  const { externalAPI = null, vibeVisible = false } = opts;

  const dom = new JSDOM(html, { runScripts: 'outside-only', pretendToBeVisual: true });
  const { window } = dom;

  const notes = [];
  window.sdNotify = (s) => {
    try { notes.push(JSON.parse(s)); } catch { notes.push(s); }
  };

  if (externalAPI) window.externalAPI = externalAPI;

  window.Element.prototype.getBoundingClientRect = function () {
    let visible = false;
    try {
      visible = vibeVisible && typeof this.matches === 'function'
        && this.matches('[class*="VibePlayerControls_root"]');
    } catch { visible = false; }
    const w = visible ? 200 : 0;
    const h = visible ? 50 : 0;
    return { width: w, height: h, top: 0, left: 0, right: w, bottom: h, x: 0, y: 0, toJSON() { return {}; } };
  };

  const proto = window.HTMLInputElement.prototype;
  const desc = Object.getOwnPropertyDescriptor(proto, 'value');
  const setCalls = [];
  Object.defineProperty(proto, 'value', {
    configurable: true,
    enumerable: desc.enumerable,
    get: desc.get,
    set(v) { setCalls.push(Number(v)); desc.set.call(this, v); },
  });

  const ret = window.eval(SRC);
  const ctrl = window._PyYMController;
  if (ctrl && typeof ctrl.stopObservation === 'function') ctrl.stopObservation();
  notes.length = 0;

  return { window, dom, ctrl, notes, setCalls, ret };
}

export function close(env) {
  try { env?.ctrl?.stopObservation?.(); } catch {}
  try { env?.dom?.window?.close(); } catch {}
}

export function clickSpy(el) {
  const calls = { count: 0 };
  el.addEventListener('click', () => { calls.count += 1; });
  return calls;
}

export const delay = (ms) => new Promise((r) => setTimeout(r, ms));
