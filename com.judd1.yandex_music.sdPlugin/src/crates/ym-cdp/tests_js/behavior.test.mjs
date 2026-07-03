import { test, describe, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mount, close } from './helpers/load.mjs';
import { SONATA_HTML } from './fixtures/sonata.mjs';

describe('эталон behaviors (selector-independent)', () => {
  let env;
  afterEach(() => { close(env); env = null; });

  test('cover URL is upscaled with a targeted segment swap to /400x400', () => {
    env = mount(SONATA_HTML);
    const cover = env.ctrl.getFullState().data.track.cover;
    assert.ok(cover.includes('/400x400'));
    assert.ok(!cover.includes('/100x100'));
  });

  test('readVolume no longer leaks the internal method field', () => {
    env = mount(SONATA_HTML);
    const vol = env.ctrl.getFullState().data.volume;
    assert.deepEqual(Object.keys(vol).sort(), ['current', 'is_muted']);
  });

  test('changeVolume never moves the timecode slider (Sonata R-guard)', () => {
    env = mount(SONATA_HTML);
    const seek = env.window.document.querySelector("[data-test-id='TIMECODE_SLIDER']");
    const before = seek.value;
    env.ctrl.changeVolume('SET', 70);
    assert.equal(seek.value, before);
  });

  test('volume write scales to the slider max (max=100 slider)', () => {
    const HTML = `<!DOCTYPE html><html><body>
      <div data-test-id="PLAYERBAR_DESKTOP">
        <div class="ChangeVolume_root__v">
          <input type="range" data-test-id="VOLUME_SLIDER" min="0" max="100" step="1" value="20">
        </div>
      </div>
    </body></html>`;
    env = mount(HTML);
    const slider = env.window.document.querySelector("[data-test-id='VOLUME_SLIDER']");
    const r = env.ctrl.changeVolume('SET', 50);
    assert.equal(r.volume, 50);
    assert.equal(slider.value, '50');
  });
});
