import { test, describe, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mount, close, delay } from './helpers/load.mjs';
import { SONATA_HTML, EMPTY_HTML } from './fixtures/sonata.mjs';

const METHODS = [
  'getFullState', 'playPause', 'next', 'prev',
  'toggleLike', 'toggleDislike', 'changeVolume',
  'startObservation', 'stopObservation',
];

const STATE_SHAPE = {
  track: ['id', 'title', 'artist', 'cover'],
  state: ['playing', 'liked', 'disliked'],
  progress: ['now_sec', 'total_sec', 'ratio'],
  volume: ['current', 'is_muted'],
};

describe('contract: JS<->Python boundary (must hold across the refactor)', () => {
  let env;
  afterEach(() => { close(env); env = null; });

  test('injection: IIFE assigns truthy controller and does not throw', () => {
    env = mount(SONATA_HTML);
    assert.equal(env.ret, true);
    assert.ok(env.window._PyYMController);
    for (const m of METHODS) {
      assert.equal(typeof env.ctrl[m], 'function', `method ${m} must exist`);
    }
  });

  test('getFullState: success + exact state tree the Python side consumes', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    const d = r.data;
    for (const section of Object.keys(STATE_SHAPE)) {
      assert.ok(section in d, `data.${section} missing`);
      for (const leaf of STATE_SHAPE[section]) {
        assert.ok(leaf in d[section], `data.${section}.${leaf} missing`);
      }
    }
    assert.equal(typeof d.track.title, 'string');
    assert.ok(d.track.cover && d.track.cover.length > 0);
    assert.equal(typeof d.state.playing, 'boolean');
  });

  test('playPause returns {success, is_playing:boolean}', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.playPause();
    assert.equal(r.success, true);
    assert.equal(typeof r.is_playing, 'boolean');
  });

  test('next / prev return {success:true}', () => {
    env = mount(SONATA_HTML);
    assert.equal(env.ctrl.next().success, true);
    assert.equal(env.ctrl.prev().success, true);
  });

  test('toggleLike sends new_state (and not is_disliked)', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.toggleLike();
    assert.equal(r.success, true);
    assert.ok('new_state' in r);
    assert.equal(typeof r.new_state, 'boolean');
    assert.equal('is_disliked' in r, false);
  });

  test('toggleDislike sends is_disliked (load-bearing asymmetry, not new_state)', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.toggleDislike();
    assert.equal(r.success, true);
    assert.ok('is_disliked' in r);
    assert.equal(typeof r.is_disliked, 'boolean');
    assert.equal('new_state' in r, false);
  });

  test('changeVolume SET returns integer percent 0-100', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.changeVolume('SET', 50);
    assert.equal(r.success, true);
    assert.equal(r.volume, 50);
    assert.ok(Number.isInteger(r.volume));
  });

  test('changeVolume UP / DOWN step by 5 percent', () => {
    env = mount(SONATA_HTML);
    assert.equal(env.ctrl.changeVolume('UP').volume, 55);
    assert.equal(env.ctrl.changeVolume('DOWN').volume, 50);
  });

  test('changeVolume SET clamps to 0..100', () => {
    env = mount(SONATA_HTML);
    assert.equal(env.ctrl.changeVolume('SET', 130).volume, 100);
    assert.equal(env.ctrl.changeVolume('SET', -10).volume, 0);
  });

  test('changeVolume MUTE returns success without volume (no optimistic volume)', () => {
    env = mount(SONATA_HTML);
    const r = env.ctrl.changeVolume('MUTE');
    assert.equal(r.success, true);
    assert.equal(r.volume, undefined);
  });

  test('no player surface -> {success:false, reason:BAR_NOT_FOUND}', () => {
    env = mount(EMPTY_HTML);
    const r = env.ctrl.getFullState();
    assert.equal(r.success, false);
    assert.equal(r.reason, 'BAR_NOT_FOUND');
  });

  test('startObservation: first emit is FULL_STATE with the bare data object; idempotent', () => {
    env = mount(SONATA_HTML);
    env.ctrl.startObservation();
    const full = env.notes.filter((n) => n.type === 'FULL_STATE');
    assert.ok(full.length >= 1);
    assert.equal(full[0].type, 'FULL_STATE');
    assert.deepEqual(full[0].payload, JSON.parse(JSON.stringify(env.ctrl.getFullState().data)));
    const before = env.notes.filter((n) => n.type === 'FULL_STATE').length;
    env.ctrl.startObservation();
    const after = env.notes.filter((n) => n.type === 'FULL_STATE').length;
    assert.equal(after, before);
  });

  test('a state change pushes a leaf-granular DELTA', async () => {
    env = mount(SONATA_HTML);
    env.ctrl.startObservation();
    env.window.document.querySelector("[data-test-id='LIKE_BUTTON']").setAttribute('aria-pressed', 'true');
    await delay(400);
    const deltas = env.notes.filter((n) => n.type === 'DELTA');
    assert.ok(deltas.length >= 1, 'expected at least one DELTA');
    const likedDelta = deltas.find((n) => n.payload && n.payload.state && 'liked' in n.payload.state);
    assert.ok(likedDelta, 'expected a DELTA carrying state.liked');
    assert.equal(likedDelta.payload.state.liked, true);
  });
});
