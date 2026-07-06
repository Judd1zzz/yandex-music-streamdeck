(function () {
  'use strict';

  const THROTTLE_MS = 200;
  const PROGRESS_MS = 500;
  const VOL_STEP = 0.05;

  const VIBE_GATE = "[class*='VibePlayerControls_root']";

  const ROOT = {
    sonata: ["[data-test-id='PLAYERBAR_DESKTOP']", "div[class*='PlayerBarDesktop']", "section[class*='PlayerBarDesktop']"],
    vibe: ["[data-test-id='VIBE_PLAYERBAR']", "[class*='VibePlayerBar_root']"],
  };

  const SEL = {
    title: {
      vibe: ["[data-test-id='VIBE_PLAYERBAR_TRACK_NAME'] [class*='trackNameText']", "[class*='VibePlayerbarMeta_trackNameText']", "[data-test-id='VIBE_PLAYERBAR_TRACK_NAME'] div"],
      sonata: ["[data-test-id='TRACK_TITLE']", "[class*='PlayerBarTitle_title']", "a[href*='/track/']"],
    },
    artist: {
      sonata: ["[data-test-id='SEPARATED_ARTIST_TITLE']", "[class*='PlayerBarTitle_artist']", "a[href*='/artist']"],
    },
    cover: {
      vibe: ["[data-test-id='VIBE_ALBUM_COVER'] img", "[class*='AlbumCover_cover'] img", "[class*='AlbumCover_root'] img"],
      sonata: ["img[data-test-id='ENTITY_COVER_IMAGE']", "[class*='PlayerBarDesktop_cover'] img"],
    },
    pauseIndicator: {
      vibe: ["[data-test-id='PAUSE_BUTTON']"],
      sonata: ["[data-test-id='PAUSE_BUTTON']", "button[class*='pause']"],
    },
    play: {
      vibe: { sel: ["[data-test-id='PAUSE_BUTTON']", "[data-test-id='PLAY_BUTTON']", "[class*='VibePlayerControls_playButton']"], button: true },
      sonata: { sel: ["[data-test-id='PAUSE_BUTTON']", "[data-test-id='PLAY_BUTTON']", "button[class*='pause']", "button[class*='play']"], button: true },
    },
    next: {
      vibe: { sel: ["[data-test-id='NEXT_TRACK_BUTTON']"], button: true },
      sonata: { sel: ["[data-test-id='NEXT_TRACK_BUTTON']", "button[aria-label*='Next']", "button[class*='next']"], button: true },
    },
    prev: {
      vibe: { sel: ["[data-test-id='PREVIOUS_TRACK_BUTTON']"], button: true },
      sonata: { sel: ["[data-test-id='PREVIOUS_TRACK_BUTTON']", "button[aria-label*='Prev']", "button[class*='prev']"], button: true },
    },
    like: {
      vibe: { sel: ["[data-test-id='LIKE_BUTTON']"], button: true },
      sonata: { sel: ["[data-test-id='LIKE_BUTTON']", "button[class*='like']:not([class*='dislike'])"], button: true },
    },
    dislike: {
      vibe: { sel: ["[data-test-id='DISLIKE_BUTTON']"], button: true },
      sonata: { sel: ["[data-test-id='DISLIKE_BUTTON']", "button[class*='dislike']"], button: true },
    },
    timeline: {
      vibe: ["[data-test-id='VIBE_PLAYERBAR_TIMECODE_SLIDER'] input[type='range']", "[class*='VibePlayerbarMeta_slider']"],
      sonata: ["input[data-test-id='TIMECODE_SLIDER']", "input[type='range']"],
    },
    timeNow: {
      sonata: ["[data-test-id='TIMECODE_TIME_START']"],
    },
    timeEnd: {
      sonata: ["[data-test-id='TIMECODE_TIME_END']"],
    },
    volume: {
      vibe: { scope: "[class*='ChangeVolume_root']", sel: ["input[data-test-id='CHANGE_VOLUME_SLIDER']", "input[class*='ChangeVolume_slider']", "input[type='range']"] },
      sonata: { scope: "[class*='ChangeVolume_root']", sel: ["input[data-test-id='CHANGE_VOLUME_SLIDER']", "input[data-test-id='VOLUME_SLIDER']", "input[class*='ChangeVolume_slider']", "input[type='range']"] },
    },
    mute: {
      vibe: { sel: ["[data-test-id='CHANGE_VOLUME_BUTTON']", "[class*='ChangeVolume_button']"], button: true },
      sonata: { sel: ["[data-test-id='CHANGE_VOLUME_BUTTON']", "[data-test-id='VOLUME_BUTTON']", "button[class*='ChangeVolume_button']"], button: true },
    },
  };

  function q(base, sel) {
    if (!base) return null;
    const list = Array.isArray(sel) ? sel : [sel];
    for (const s of list) {
      const el = base.querySelector(s);
      if (el) return el;
    }
    return null;
  }

  function toBtn(el) {
    if (!el) return null;
    return el.tagName === 'BUTTON' ? el : el.closest('button');
  }

  function visible(el) {
    if (!el) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  function toSec(timeStr) {
    if (!timeStr) return 0;
    const p = timeStr.split(':').map(Number);
    if (p.length === 2) return p[0] * 60 + p[1];
    if (p.length === 3) return p[0] * 3600 + p[1] * 60 + p[2];
    return 0;
  }

  function toPercent(val) {
    if (val === null || val === undefined) return 0;
    const n = parseFloat(val);
    if (isNaN(n)) return 0;
    if (n >= 0 && n <= 1) return Math.round(n * 100);
    return Math.min(Math.round(n), 100);
  }

  function isBtnActive(btn) {
    if (!btn) return false;
    const pressed = btn.getAttribute('aria-pressed');
    if (pressed !== null) return pressed === 'true';
    const cls = btn.className || '';
    if (cls.includes('active') || cls.includes('checked')) return true;
    const icon = btn.querySelector('use');
    const href = icon ? (icon.getAttribute('xlink:href') || icon.getAttribute('href') || '') : '';
    return href.includes('filled') || href.includes('liked');
  }

  function useHref(el) {
    const icon = el ? el.querySelector('use') : null;
    return icon ? (icon.getAttribute('xlink:href') || icon.getAttribute('href') || '') : '';
  }

  function checkMute(btn) {
    if (!btn) return false;
    const label = (btn.getAttribute('aria-label') || '').toLowerCase();
    if (label.includes('включить звук') || label.includes('turn on sound')) return true;
    if (label.includes('выключить звук') || label.includes('turn off sound')) return false;
    const href = useHref(btn).toLowerCase();
    if (href.includes('volumeoff') || href.includes('mute') || href.includes('_off')) return true;
    const cls = (btn.className || '').toLowerCase();
    return cls.includes('muted');
  }

  function upscaleCover(url) {
    if (!url) return url;
    if (url.indexOf('/100x100') !== -1) return url.replace('/100x100', '/400x400');
    if (url.indexOf('/200x200') !== -1) return url.replace('/200x200', '/400x400');
    return url;
  }

  function reactSet(input, val) {
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
    if (setter) setter.call(input, val);
    else input.value = val;
    input.dispatchEvent(new InputEvent('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }

  function fiberOf(el) {
    if (!el) return null;
    const k = Object.keys(el).find((n) => n.indexOf('__reactFiber$') === 0 || n.indexOf('__reactInternalInstance$') === 0);
    return k ? el[k] : null;
  }

  function isTrackObj(v) {
    return v && typeof v === 'object' && v.id != null && /^\d+$/.test(String(v.id)) && typeof v.title === 'string' && Array.isArray(v.artists);
  }

  function coverFromStore(uri) {
    if (typeof uri !== 'string' || !uri) return undefined;
    let u = uri.replace('%%', '400x400');
    if (u.indexOf('http') !== 0) u = 'https://' + u;
    return u;
  }

  function playerStoreSnapshot() {
    try {
      let found = null;
      const scan = (o, depth) => {
        if (found || !o || typeof o !== 'object' || depth > 4) return;
        if (o.sonataState && typeof o.sonataState === 'object' && isTrackObj(o.sonataState.entityMeta)) {
          const s = o.sonataState;
          const em = s.entityMeta;
          const names = (Array.isArray(em.artists) ? em.artists : []).map((a) => a && a.name).filter(Boolean);
          found = {
            id: String(em.id),
            title: typeof em.title === 'string' ? em.title : '',
            artist: names.join(', '),
            cover: coverFromStore(em.coverUri),
            status: typeof s.status === 'string' ? s.status : null,
            position: typeof s.position === 'number' ? s.position : null,
            duration: typeof s.duration === 'number' ? s.duration : null,
            volume: typeof s.volume === 'number' ? s.volume : null,
          };
          return;
        }
        for (const k in o) {
          if (found) return;
          let v;
          try { v = o[k]; } catch (e) { continue; }
          if (v && typeof v === 'object') scan(v, depth + 1);
        }
      };
      const stack = [fiberOf(document.body)];
      const visited = new Set();
      let seen = 0;
      while (stack.length && !found && seen < 8000) {
        const cur = stack.pop();
        if (!cur || typeof cur !== 'object' || visited.has(cur)) continue;
        visited.add(cur);
        seen += 1;
        if (cur.memoizedProps) scan(cur.memoizedProps, 0);
        if (!found && cur.memoizedState) scan(cur.memoizedState, 0);
        if (cur.child) stack.push(cur.child);
        if (cur.sibling) stack.push(cur.sibling);
      }
      return found;
    } catch (e) {
      return null;
    }
  }

  function resolveTrackId(root, titleEl) {
    const regex = /[?&]trackId=(\d+)|track\/(\d+)/;
    const sources = [
      titleEl ? titleEl.href : null,
      window.location.href,
      root ? (root.querySelector("a[href*='/track/']") || {}).href : null,
    ];
    for (const src of sources) {
      if (!src) continue;
      const m = String(src).match(regex);
      if (m) return m[1] || m[2];
    }
    try {
      if (window.externalAPI && typeof window.externalAPI.getCurrentTrack === 'function') {
        const t = window.externalAPI.getCurrentTrack();
        if (t && t.id != null && String(t.id)) return String(t.id);
      }
    } catch (e) {}
    return null;
  }

  function surfaceRoot(surface, ctx) {
    return surface === 'vibe' ? ctx.vibeRoot : ctx.sonataRoot;
  }

  function buildCtx() {
    const isVibe = visible(document.querySelector(VIBE_GATE));
    const sonataRoot = q(document, ROOT.sonata);
    const vibeRoot = q(document, ROOT.vibe);
    const activeRoot = isVibe ? (vibeRoot || sonataRoot) : (sonataRoot || vibeRoot);
    return { isVibe, sonataRoot, vibeRoot, activeRoot, hasSurface: !!activeRoot };
  }

  function resolve(ctx, control) {
    const d = SEL[control];
    if (!d) return null;
    const order = ctx.isVibe ? ['vibe', 'sonata'] : ['sonata', 'vibe'];
    for (const surface of order) {
      const raw = d[surface];
      if (!raw) continue;
      const spec = Array.isArray(raw) ? { sel: raw } : raw;
      const base = spec.scope ? document.querySelector(spec.scope) : surfaceRoot(surface, ctx);
      if (!base) continue;
      const el = q(base, spec.sel);
      if (el) return spec.button ? toBtn(el) : el;
    }
    return null;
  }

  function isPlaying(ctx) {
    return !!resolve(ctx, 'pauseIndicator');
  }

  function readArtist(ctx) {
    if (ctx.isVibe) {
      const scope = document.querySelector("[class*='VibePage_entityMeta']");
      if (scope) {
        const links = scope.querySelectorAll("[data-test-id='SEPARATED_ARTIST_TITLE'], a[href*='/artist']");
        const names = [];
        links.forEach((el) => {
          const name = (el.textContent || '').trim();
          if (name && names.indexOf(name) === -1) names.push(name);
        });
        if (names.length) return names.join(', ');
      }
    }
    const el = resolve(ctx, 'artist');
    return (el && el.textContent ? el.textContent.trim() : '');
  }

  function readTrack(ctx, store) {
    const meta = store === undefined ? playerStoreSnapshot() : store;
    const titleEl = resolve(ctx, 'title');
    const coverEl = resolve(ctx, 'cover');
    let domCover = coverEl ? coverEl.currentSrc || coverEl.src : undefined;
    if (domCover) domCover = upscaleCover(domCover);

    let title = meta && meta.title ? meta.title : '';
    let artist = meta && meta.artist ? meta.artist : '';
    if (!title || !artist) {
      const rawTitle = (titleEl && titleEl.textContent ? titleEl.textContent.trim() : '');
      let dTitle = rawTitle;
      let dArtist = readArtist(ctx);
      if (!dArtist && rawTitle) {
        const parts = rawTitle.split(/\s+—\s+/);
        if (parts.length >= 2) {
          dArtist = parts[0].trim();
          dTitle = parts.slice(1).join(' — ').trim();
        }
      }
      if (!title) title = dTitle;
      if (!artist) artist = dArtist;
    }

    return {
      id: (meta && meta.id) || resolveTrackId(ctx.activeRoot || document, titleEl),
      title: title || 'Unknown',
      artist: artist || 'Unknown',
      cover: (meta && meta.cover) || domCover,
    };
  }

  function readState(ctx, store) {
    const s = store === undefined ? playerStoreSnapshot() : store;
    return {
      playing: s && s.status ? s.status === 'playing' : isPlaying(ctx),
      liked: isBtnActive(resolve(ctx, 'like')),
      disliked: isBtnActive(resolve(ctx, 'dislike')),
    };
  }

  function readProgress(ctx, store) {
    const s = store === undefined ? playerStoreSnapshot() : store;
    if (s && typeof s.position === 'number' && typeof s.duration === 'number' && s.duration > 0) {
      const ratio = s.position / s.duration;
      return { now_sec: s.position, total_sec: s.duration, ratio: isFinite(ratio) ? ratio : 0 };
    }
    let now = 0;
    let duration = 0;
    let progress = 0;
    try {
      if (window.externalAPI) {
        const rawProgress = window.externalAPI.getProgress();
        const rawDuration = window.externalAPI.getDuration();
        now = typeof rawProgress === 'number' && !isNaN(rawProgress) ? rawProgress : 0;
        duration = typeof rawDuration === 'number' && !isNaN(rawDuration) ? rawDuration : 0;
        progress = duration > 0 ? now / duration : 0;
      } else {
        const slider = resolve(ctx, 'timeline');
        if (slider) {
          const val = parseFloat(slider.value) || 0;
          const max = parseFloat(slider.max) || 0;
          now = val;
          duration = max;
          progress = max > 0 ? val / max : 0;
        } else {
          const nowEl = resolve(ctx, 'timeNow');
          const endEl = resolve(ctx, 'timeEnd');
          now = toSec(nowEl ? nowEl.textContent : '0:00');
          duration = toSec(endEl ? endEl.textContent : '0:00');
          progress = duration > 0 ? now / duration : 0;
        }
      }
    } catch (e) {}
    return { now_sec: now, total_sec: duration, ratio: isNaN(progress) ? 0 : isFinite(progress) ? progress : 0 };
  }

  function readVolume(ctx, store) {
    const s = store === undefined ? playerStoreSnapshot() : store;
    if (s && typeof s.volume === 'number') {
      return { current: toPercent(s.volume), is_muted: checkMute(resolve(ctx, 'mute')) };
    }
    if (window.externalAPI && typeof window.externalAPI.getVolume === 'function') {
      try {
        const apiVol = window.externalAPI.getVolume();
        let isMuted = false;
        if (typeof window.externalAPI.getMute === 'function') isMuted = window.externalAPI.getMute();
        else isMuted = checkMute(resolve(ctx, 'mute'));
        return { current: toPercent(apiVol), is_muted: isMuted };
      } catch (e) {}
    }
    const slider = resolve(ctx, 'volume');
    if (slider) {
      const val = parseFloat(slider.value);
      const max = parseFloat(slider.max) || 1;
      const ratio = max <= 1 ? val : val / max;
      const isMuted = checkMute(resolve(ctx, 'mute'));
      return { current: toPercent(ratio), is_muted: isMuted };
    }
    return { current: 0, is_muted: false };
  }

  function setVolume(ctx, val) {
    const ratio = Math.max(0, Math.min(1, Math.round(val * 100) / 100));
    if (window.externalAPI && typeof window.externalAPI.setVolume === 'function') {
      try {
        window.externalAPI.setVolume(ratio);
        return { success: true, volume: Math.round(ratio * 100) };
      } catch (e) {}
    }
    const slider = resolve(ctx, 'volume');
    if (slider) {
      const max = parseFloat(slider.max) || 1;
      reactSet(slider, max <= 1 ? ratio : ratio * max);
      return { success: true, volume: Math.round(ratio * 100) };
    }
    return { success: false, error: 'Volume control unavailable' };
  }

  function toggleMute(ctx) {
    if (window.externalAPI && typeof window.externalAPI.toggleMute === 'function') {
      window.externalAPI.toggleMute();
      return { success: true };
    }
    const btn = resolve(ctx, 'mute');
    if (btn) {
      btn.click();
      return { success: true };
    }
    return { success: false, error: 'Mute button unavailable' };
  }

  function deepDiff(a, b) {
    if (a === b) return undefined;
    if (typeof a !== typeof b || a === null || b === null) return b;
    if (Array.isArray(a)) {
      if (!Array.isArray(b) || a.length !== b.length) return b;
      for (let i = 0; i < a.length; i++) {
        if (JSON.stringify(a[i]) !== JSON.stringify(b[i])) return b;
      }
      return undefined;
    }
    if (typeof a === 'object') {
      const diff = {};
      let changed = false;
      for (const key in b) {
        if (!(key in a)) {
          diff[key] = b[key];
          changed = true;
        } else {
          const d = deepDiff(a[key], b[key]);
          if (d !== undefined) {
            diff[key] = d;
            changed = true;
          }
        }
      }
      return changed ? diff : undefined;
    }
    return b;
  }

  class YMController {
    constructor() {
      this.observing = false;
      this.lastState = null;
      this._observer = null;
      this._tick = null;
      this._throttle = null;
      this._volInput = null;
      this._dlClick = null;
    }

    getFullState() {
      const ctx = buildCtx();
      if (!ctx.hasSurface) return { success: false, reason: 'BAR_NOT_FOUND' };
      try {
        const store = playerStoreSnapshot();
        return {
          success: true,
          data: {
            track: readTrack(ctx, store),
            state: readState(ctx, store),
            progress: readProgress(ctx, store),
            volume: readVolume(ctx, store),
          },
        };
      } catch (e) {
        return { success: false, error: e.toString() };
      }
    }

    playPause() {
      const ctx = buildCtx();
      const btn = resolve(ctx, 'play');
      if (!btn) return { success: false };
      const wasPlaying = isPlaying(ctx);
      btn.click();
      return { success: true, is_playing: !wasPlaying };
    }

    _clickSimple(control) {
      const ctx = buildCtx();
      const btn = resolve(ctx, control);
      if (!btn || btn.disabled) return { success: false };
      btn.click();
      return { success: true };
    }

    next() {
      return this._clickSimple('next');
    }

    prev() {
      return this._clickSimple('prev');
    }

    toggleLike() {
      const ctx = buildCtx();
      const btn = resolve(ctx, 'like');
      if (!btn) return { success: false };
      const was = isBtnActive(btn);
      btn.click();
      return { success: true, new_state: !was };
    }

    toggleDislike() {
      const ctx = buildCtx();
      const btn = resolve(ctx, 'dislike');
      if (!btn) return { success: false };
      const was = isBtnActive(btn);
      btn.click();
      return { success: true, is_disliked: !was };
    }

    changeVolume(action, value) {
      try {
        const ctx = buildCtx();
        const current = readVolume(ctx).current / 100;
        let result;
        switch (action) {
          case 'UP':
            result = setVolume(ctx, current + VOL_STEP);
            break;
          case 'DOWN':
            result = setVolume(ctx, current - VOL_STEP);
            break;
          case 'SET':
            result = setVolume(ctx, value / 100);
            break;
          case 'MUTE':
            return toggleMute(ctx);
          default:
            return { success: false, error: 'Unknown action' };
        }
        this._updateVolumeBadge();
        return result;
      } catch (e) {
        return { success: false, error: e.toString() };
      }
    }

    startObservation() {
      if (this.observing) return;
      this.observing = true;
      this.lastState = null;
      const self = this;
      this._observer = new MutationObserver(function () { self._schedule(); });
      this._observer.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
      this._tick = setInterval(function () {
        if (self.lastState && self.lastState.state && self.lastState.state.playing) self._emit();
      }, PROGRESS_MS);
      this._volInput = function (e) {
        const t = e.target;
        if (t && t.matches && t.matches("[data-test-id='CHANGE_VOLUME_SLIDER'], input[class*='ChangeVolume_slider']")) self._updateVolumeBadge();
      };
      document.body.addEventListener('input', this._volInput, true);
      this._dlClick = function (e) {
        const btn = e.target && e.target.closest ? e.target.closest('.ym-dl-btn') : null;
        if (!btn) return;
        e.stopPropagation();
        e.preventDefault();
        self.downloadCurrent();
        self._flashDownloadButton(btn);
      };
      document.body.addEventListener('click', this._dlClick, true);
      this._emit();
    }

    stopObservation() {
      this.observing = false;
      if (this._observer) {
        this._observer.disconnect();
        this._observer = null;
      }
      if (this._tick) {
        clearInterval(this._tick);
        this._tick = null;
      }
      if (this._throttle) {
        clearTimeout(this._throttle);
        this._throttle = null;
      }
      if (this._volInput) {
        document.body.removeEventListener('input', this._volInput, true);
        this._volInput = null;
      }
      if (this._dlClick) {
        document.body.removeEventListener('click', this._dlClick, true);
        this._dlClick = null;
      }
    }

    _schedule() {
      if (this._throttle) return;
      const self = this;
      this._throttle = setTimeout(function () {
        self._throttle = null;
        self._emit();
      }, THROTTLE_MS);
    }

    _emit() {
      if (!this.observing) return;
      try {
        const raw = this.getFullState();
        if (!raw || !raw.success) return;
        this._updateVolumeBadge();
        this._updateDownloadButton();
        if (!this.lastState) {
          this.lastState = raw.data;
          this._notify('FULL_STATE', raw.data);
          return;
        }
        const delta = deepDiff(this.lastState, raw.data);
        if (delta) {
          this.lastState = raw.data;
          this._notify('DELTA', delta);
        }
      } catch (e) {}
    }

    _notify(type, payload) {
      if (window.sdNotify) window.sdNotify(JSON.stringify({ type: type, payload: payload }));
    }

    _updateVolumeBadge() {
      try {
        const ctx = buildCtx();
        const slider = resolve(ctx, 'volume');
        if (!slider) return;
        const box = slider.closest("[class*='ChangeVolume_wrapperSlider']");
        const host = box || slider.closest("[class*='ChangeVolume_sliderContainer']") || slider.closest("[class*='ChangeVolume_root']");
        if (!host) return;
        let badge = host.querySelector('.ym-vol-pct');
        if (!badge) {
          badge = document.createElement('div');
          badge.className = 'ym-vol-pct';
          badge.style.cssText = 'position:absolute;left:50%;transform:translateX(-50%);font-size:12px;font-weight:600;line-height:1;color:#fff;pointer-events:none;z-index:5;text-shadow:0 1px 2px rgba(0,0,0,0.6);white-space:nowrap;';
          host.insertBefore(badge, host.firstChild);
        }
        if (box) {
          if (window.getComputedStyle(box).position === 'static') box.style.position = 'relative';
          const sr = slider.getBoundingClientRect();
          const wr = box.getBoundingClientRect();
          if (sr.height > 0) {
            const gap = Math.round(wr.bottom - sr.bottom);
            if (gap > 0) {
              const bh = Math.round(badge.getBoundingClientRect().height) || 12;
              const border = Math.round(parseFloat(window.getComputedStyle(box).borderTopWidth)) || 0;
              const h = Math.round(wr.bottom - sr.top + gap * 2 + bh) + 'px';
              if (box.style.minHeight !== h) box.style.minHeight = h;
              const top = gap - border + 'px';
              if (badge.style.top !== top) badge.style.top = top;
            }
          }
        }
        const text = readVolume(ctx).current + '%';
        if (badge.textContent !== text) badge.textContent = text;
      } catch (e) {}
    }

    downloadCurrent() {
      try {
        const ctx = buildCtx();
        const track = readTrack(ctx);
        if (!track || !track.id) return { success: false, error: 'No track id' };
        this._notify('DOWNLOAD', { track_id: String(track.id) });
        return { success: true };
      } catch (e) {
        return { success: false, error: e.toString() };
      }
    }

    _flashDownloadButton(btn) {
      btn.style.opacity = '0.45';
      setTimeout(function () { btn.style.opacity = '1'; }, 1400);
    }

    _updateDownloadButton() {
      try {
        if (!document.getElementById('ym-dl-style')) {
          const st = document.createElement('style');
          st.id = 'ym-dl-style';
          st.textContent = '.ym-dl-btn{transition:color .15s ease,opacity .15s ease}.ym-dl-btn:hover{color:#fff !important;opacity:1 !important}';
          (document.head || document.documentElement).appendChild(st);
        }
        const ctx = buildCtx();
        if (!ctx.hasSurface) return;
        const like = resolve(ctx, 'like');
        if (!like || !like.parentElement) return;
        const host = like.parentElement;
        const cs = window.getComputedStyle(like);
        const existing = host.querySelector('.ym-dl-btn');
        if (existing) {
          if (existing.style.color !== cs.color) existing.style.color = cs.color;
          return;
        }
        const nsvg = like.querySelector('svg');
        const sw = nsvg ? window.getComputedStyle(nsvg).width : '24px';
        const sh = nsvg ? window.getComputedStyle(nsvg).height : '24px';
        const btn = document.createElement('button');
        btn.className = 'ym-dl-btn';
        btn.type = 'button';
        btn.setAttribute('aria-label', 'Скачать трек');
        btn.title = 'Скачать трек';
        btn.style.cssText = [
          'box-sizing:border-box', 'cursor:pointer', 'display:inline-flex', 'align-items:center', 'justify-content:center', 'opacity:1',
          'position:relative', 'z-index:1', 'pointer-events:auto',
          'background:' + cs.backgroundColor, 'border:' + cs.border, 'border-radius:' + cs.borderRadius,
          'width:' + cs.width, 'height:' + cs.height, 'padding:' + cs.padding, 'margin:' + cs.margin, 'color:' + cs.color,
        ].join(';');
        btn.innerHTML = "<svg width='" + sw + "' height='" + sh + "' viewBox='0 0 24 24' fill='currentColor' aria-hidden='true'><path d='M12 3a1 1 0 0 1 1 1v8.59l2.3-2.3a1 1 0 1 1 1.42 1.42l-4 4a1 1 0 0 1-1.42 0l-4-4a1 1 0 1 1 1.42-1.42l2.3 2.3V4a1 1 0 0 1 1-1zM5 19a1 1 0 1 0 0 2h14a1 1 0 1 0 0-2z'/></svg>";
        host.insertBefore(btn, like.nextSibling);
      } catch (e) {}
    }
  }

  const ctrl = new YMController();
  window._PyYMController = ctrl;
  try {
    ctrl.startObservation();
  } catch (e) {}

  return true;
})();
