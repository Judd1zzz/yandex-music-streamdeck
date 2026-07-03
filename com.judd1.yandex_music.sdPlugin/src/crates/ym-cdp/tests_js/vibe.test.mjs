import { test, describe, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mount, close, clickSpy } from './helpers/load.mjs';
import { VIBE_HTML, SONATA_PLUS_HIDDEN_VIBE_HTML, VIBE_MY_VIBE_PAUSED_HTML } from './fixtures/vibe.mjs';

describe('Vibe surface (current Yandex Music, verified against live DOM)', () => {
  let env;
  afterEach(() => { close(env); env = null; });

  test('getFullState reads the Vibe player bar when its controls are visible', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    assert.equal(r.data.track.title, 'Faded');
    assert.equal(r.data.track.artist, 'Madonna');
    assert.equal(r.data.state.playing, true);
    assert.equal(r.data.state.liked, false);
    assert.equal(r.data.state.disliked, false);
    assert.equal(r.data.volume.current, 50);
    assert.equal(r.data.progress.now_sec, 38);
    assert.equal(r.data.progress.total_sec, 179);
    assert.ok(r.data.track.cover.includes('/400x400'));
  });

  test('playPause clicks the bar play/pause button and reports the flipped state', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const spy = clickSpy(env.window.document.querySelector("[data-test-id='PAUSE_BUTTON']"));
    const r = env.ctrl.playPause();
    assert.equal(spy.count, 1);
    assert.equal(r.is_playing, false);
  });

  test('next / prev click the data-test-id buttons inside the bar', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const nextSpy = clickSpy(env.window.document.querySelector("[data-test-id='NEXT_TRACK_BUTTON']"));
    const prevSpy = clickSpy(env.window.document.querySelector("[data-test-id='PREVIOUS_TRACK_BUTTON']"));
    assert.equal(env.ctrl.next().success, true);
    assert.equal(env.ctrl.prev().success, true);
    assert.equal(nextSpy.count, 1);
    assert.equal(prevSpy.count, 1);
  });

  test('toggleLike / toggleDislike click the right buttons and keep the contract keys', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const likeSpy = clickSpy(env.window.document.querySelector("[data-test-id='LIKE_BUTTON']"));
    const dislikeSpy = clickSpy(env.window.document.querySelector("[data-test-id='DISLIKE_BUTTON']"));
    const like = env.ctrl.toggleLike();
    const dislike = env.ctrl.toggleDislike();
    assert.equal(likeSpy.count, 1);
    assert.equal(dislikeSpy.count, 1);
    assert.equal(like.new_state, true);
    assert.equal(dislike.is_disliked, true);
  });

  test('changeVolume SET targets CHANGE_VOLUME_SLIDER, never the timecode slider (R-guard)', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;
    const volSlider = doc.querySelector("[data-test-id='CHANGE_VOLUME_SLIDER']");
    const seekSlider = doc.querySelector("[data-test-id='VIBE_PLAYERBAR_TIMECODE_SLIDER'] input[type='range']");
    const seekBefore = seekSlider.value;
    const r = env.ctrl.changeVolume('SET', 60);
    assert.equal(r.volume, 60);
    assert.equal(volSlider.value, '0.6');
    assert.equal(seekSlider.value, seekBefore);
  });

  test('mute toggle clicks CHANGE_VOLUME_BUTTON', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const muteSpy = clickSpy(env.window.document.querySelector("[data-test-id='CHANGE_VOLUME_BUTTON']"));
    const r = env.ctrl.changeVolume('MUTE');
    assert.equal(r.success, true);
    assert.equal(muteSpy.count, 1);
    assert.equal(r.volume, undefined);
  });

  test('volume % badge: создаётся один раз внутри видимого бокса слайдера и обновляется по уровню', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;

    env.ctrl._updateVolumeBadge();
    const badges = doc.querySelectorAll('.ym-vol-pct');
    assert.equal(badges.length, 1);
    const box = doc.querySelector("[class*='ChangeVolume_wrapperSlider']");
    assert.equal(badges[0].parentElement, box);
    assert.equal(badges[0].textContent, '50%');

    env.ctrl._updateVolumeBadge();
    assert.equal(doc.querySelectorAll('.ym-vol-pct').length, 1);

    doc.querySelector("[data-test-id='CHANGE_VOLUME_SLIDER']").value = '0.3';
    env.ctrl._updateVolumeBadge();
    assert.equal(doc.querySelector('.ym-vol-pct').textContent, '30%');
  });

  test('volume % badge: мгновенно обновляется по input-событию слайдера', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;
    env.ctrl.startObservation();
    const slider = doc.querySelector("[data-test-id='CHANGE_VOLUME_SLIDER']");
    slider.value = '0.4';
    slider.dispatchEvent(new env.window.Event('input', { bubbles: true }));
    assert.equal(doc.querySelector('.ym-vol-pct').textContent, '40%');
    env.ctrl.stopObservation();
  });

  test('download button: вставляется рядом с лайком, делегированный клик шлёт DOWNLOAD с track_id', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;
    doc.querySelector("[data-test-id='VIBE_PLAYERBAR']").insertAdjacentHTML('beforeend', '<a href="https://music.yandex.ru/track/12345"></a>');
    env.ctrl.startObservation();
    env.ctrl._updateDownloadButton();
    const btns = doc.querySelectorAll('.ym-dl-btn');
    assert.equal(btns.length, 1);
    // кнопка поднята над оверлеем фонового прогресса плеер-бара
    assert.equal(btns[0].style.position, 'relative');
    assert.equal(btns[0].style.zIndex, '1');
    // hover-эффект (иконка белеет при наведении, как у соседних кнопок)
    const dlStyle = doc.getElementById('ym-dl-style');
    assert.ok(dlStyle, 'инжектится <style> для hover');
    assert.match(dlStyle.textContent, /\.ym-dl-btn:hover/);
    assert.match(dlStyle.textContent, /#fff/);
    const like = doc.querySelector("[data-test-id='LIKE_BUTTON']");
    assert.equal(btns[0].parentElement, like.parentElement);

    env.ctrl._updateDownloadButton();
    assert.equal(doc.querySelectorAll('.ym-dl-btn').length, 1);

    const icon = btns[0].querySelector('svg');
    icon.dispatchEvent(new env.window.MouseEvent('click', { bubbles: true }));
    const dl = env.notes.find((n) => n && n.type === 'DOWNLOAD');
    assert.ok(dl, 'делегированный клик (даже по иконке) должен отправить DOWNLOAD');
    assert.equal(dl.payload.track_id, '12345');
    env.ctrl.stopObservation();
  });

  test('track id/title/artist/cover берутся из стора и бьют DOM (Vibe)', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;
    const fiber = {
      memoizedProps: {
        value: {
          sonataState: {
            entityMeta: {
              id: '987654',
              title: 'StoreTitle',
              artists: [{ name: 'A' }, { name: 'B' }],
              coverUri: 'avatars.yandex.net/get-music-content/x/y/%%',
            },
          },
        },
      },
      child: null,
      sibling: null,
    };
    doc.body['__reactFiber$xyztest'] = fiber;
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    // стор бьёт DOM (в VIBE_HTML — Faded / Madonna)
    assert.equal(r.data.track.id, '987654');
    assert.equal(r.data.track.title, 'StoreTitle');
    assert.equal(r.data.track.artist, 'A, B');
    assert.ok(r.data.track.cover.startsWith('https://avatars'));
    assert.match(r.data.track.cover, /400x400/);
    assert.ok(!r.data.track.cover.includes('%%'));
  });

  test('progress/volume/playing берутся из стора; like/dislike — из DOM (aria-pressed)', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const doc = env.window.document;
    doc.body['__reactFiber$xyztest'] = {
      memoizedProps: {
        value: {
          sonataState: {
            entityMeta: { id: '111', title: 'T', artists: [{ name: 'A' }] },
            status: 'playing',
            position: 100,
            duration: 200,
            volume: 0.3,
          },
        },
      },
      child: null,
      sibling: null,
    };
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    // прогресс/громкость/playing из стора (бьют DOM 38/179, 50)
    assert.equal(r.data.progress.now_sec, 100);
    assert.equal(r.data.progress.total_sec, 200);
    assert.equal(r.data.progress.ratio, 0.5);
    assert.equal(r.data.volume.current, 30);
    assert.equal(r.data.state.playing, true);
    // like/dislike по-прежнему из DOM (aria-pressed), стор их не трогает
    assert.equal(r.data.state.liked, false);
    assert.equal(r.data.state.disliked, false);
  });

  test('без стора title/artist берутся из DOM (фолбэк)', () => {
    env = mount(VIBE_HTML, { vibeVisible: true });
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    assert.equal(r.data.track.title, 'Faded');
    assert.equal(r.data.track.artist, 'Madonna');
  });

  test('Моя Волна на паузе: комбинированная строка "Артист — Название" даёт стабильные title/artist', () => {
    env = mount(VIBE_MY_VIBE_PAUSED_HTML, { vibeVisible: true });
    const r = env.ctrl.getFullState();
    assert.equal(r.success, true);
    // несмотря на схлопнутый плеербар и "My Vibe" в entityMeta — те же title/artist, что при игре
    assert.equal(r.data.track.title, 'Faded');
    assert.equal(r.data.track.artist, 'Madonna');
    assert.equal(r.data.state.playing, false);
  });

  test('surface gate: an invisible Vibe controls root falls back to the Sonata bar', () => {
    env = mount(SONATA_PLUS_HIDDEN_VIBE_HTML, { vibeVisible: false });
    assert.equal(env.ctrl.getFullState().data.track.title, 'Sonata Track');
    close(env);
    env = mount(SONATA_PLUS_HIDDEN_VIBE_HTML, { vibeVisible: true });
    assert.equal(env.ctrl.getFullState().data.track.title, 'Vibe Track');
  });
});
