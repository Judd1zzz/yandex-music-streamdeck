import { test, describe, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mount, close, delay } from './helpers/load.mjs';
import { SONATA_HTML } from './fixtures/sonata.mjs';

describe('observation engine (MutationObserver + progress tick)', () => {
  let env;
  afterEach(() => { close(env); env = null; });

  test('stopObservation fully halts further emits', async () => {
    env = mount(SONATA_HTML);
    env.ctrl.startObservation();
    env.notes.length = 0;
    env.ctrl.stopObservation();
    env.window.document.querySelector("[data-test-id='LIKE_BUTTON']").setAttribute('aria-pressed', 'true');
    await delay(400);
    assert.equal(env.notes.length, 0);
  });

  test('a DOM mutation pushes a DELTA through the observer', async () => {
    env = mount(SONATA_HTML);
    env.ctrl.startObservation();
    env.window.document.querySelector("[data-test-id='DISLIKE_BUTTON']").setAttribute('aria-pressed', 'true');
    await delay(400);
    const deltas = env.notes.filter((n) => n.type === 'DELTA');
    const hit = deltas.find((n) => n.payload && n.payload.state && n.payload.state.disliked === true);
    assert.ok(hit, 'expected a DELTA with state.disliked');
  });

  test('progress tick emits a DELTA while playing as externalAPI progress advances', async () => {
    let p = 0;
    const externalAPI = {
      getProgress: () => { p += 5; return p; },
      getDuration: () => 180,
      getVolume: () => 0.5,
      getMute: () => false,
    };
    env = mount(SONATA_HTML, { externalAPI });
    env.ctrl.startObservation();
    const full = env.notes.find((n) => n.type === 'FULL_STATE');
    assert.ok(full);
    const startNow = full.payload.progress.now_sec;
    await delay(700);
    const deltas = env.notes.filter((n) => n.type === 'DELTA' && n.payload.progress && 'now_sec' in n.payload.progress);
    assert.ok(deltas.length >= 1, 'expected a progress DELTA from the tick');
    assert.ok(deltas[deltas.length - 1].payload.progress.now_sec > startNow);
  });
});
