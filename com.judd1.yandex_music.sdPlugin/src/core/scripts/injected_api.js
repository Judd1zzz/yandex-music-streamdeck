(function() {
    'use strict';

    const DOM = {
        SCOPE: [
            "[data-test-id='PLAYERBAR_DESKTOP']",
            "div[class*='PlayerBarDesktop']",
            "section[class*='PlayerBarDesktop']"
        ],
        Track: {
            TITLE: ["[data-test-id='TRACK_TITLE']", "[class*='PlayerBarTitle_title']", "a[href*='/track/']"],
            ARTIST: ["[data-test-id='SEPARATED_ARTIST_TITLE']", "[class*='PlayerBarTitle_artist']", "a[href*='/artist/']"],
            COVER: ["img[data-test-id='ENTITY_COVER_IMAGE']", "[class*='PlayerBarDesktop_cover'] img"]
        },
        Controls: {
            PLAY_PAUSE: "button[class*='pause'], button[class*='play'], [data-test-id='PAUSE_BUTTON'], [data-test-id='PLAY_BUTTON']",
            NEXT: ["[data-test-id='NEXT_TRACK_BUTTON']", "button[aria-label*='Next']", "button[class*='next']"],
            PREV: ["[data-test-id='PREVIOUS_TRACK_BUTTON']", "button[aria-label*='Prev']", "button[class*='prev']"],
            LIKE: ["[data-test-id='LIKE_BUTTON']", "button[class*='like']:not([class*='dislike'])"],
            DISLIKE: ["[data-test-id='DISLIKE_BUTTON']", "button[class*='dislike']"],
            TIMELINE: "input[data-test-id='TIMECODE_SLIDER'], input[type='range']",
            TIME_NOW: "[data-test-id='TIMECODE_TIME_START']",
            TIME_END: "[data-test-id='TIMECODE_TIME_END']"
        },
        Volume: {
            SLIDER: ["input[data-test-id='VOLUME_SLIDER']", "input[class*='ChangeVolume_slider']", "div[class*='Volume'] input[type='range']"],
            MUTE_BTN: ["button[data-test-id='VOLUME_BUTTON']", "button[class*='ChangeVolume_button']", "button[class*='Volume']"]
        }
    };

    const Utils = {
        find: (root, selectors) => {
            if (!root) return null;
            const list = Array.isArray(selectors) ? selectors : [selectors];
            for (let sel of list) {
                const el = root.querySelector(sel);
                if (el) return el;
            }
            return null;
        },

        findBtn: (root, selectors) => {
            const el = Utils.find(root, selectors);
            return el ? (el.tagName === 'BUTTON' ? el : el.closest('button')) : null;
        },

        toSec: (timeStr) => {
            if (!timeStr) return 0;
            const p = timeStr.split(':').map(Number);
            return p.length === 2 ? p[0] * 60 + p[1] : (p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : 0);
        },
        
        resolveTrackId: (root, titleEl) => {
            const regex = /[?&]trackId=(\d+)|track\/(\d+)/;
            const sources = [
                titleEl?.href,
                window.location.href,
                root?.querySelector("a[href*='/track/']")?.href, 
                window.externalAPI?.getCurrentTrack()?.id 
            ];

            for (let src of sources) {
                if (!src) continue;
                const match = String(src).match(regex);
                if (match) return match[1] || match[2];
            }
            return null;
        },

        isBtnActive: (btn) => {
            if (!btn) return false;
            const pressed = btn.getAttribute('aria-pressed');
            if (pressed !== null) return pressed === 'true';
            
            const cls = btn.className;
            if (cls.includes('active') || cls.includes('checked')) return true;

            const icon = btn.querySelector('use')?.getAttribute('xlink:href') || '';
            return icon.includes('filled') || icon.includes('liked');
        },

        toPercent: (val) => {
            if (val === null || val === undefined) return 0;
            let n = parseFloat(val);
            if (isNaN(n)) return 0;
            if (n >= 0 && n <= 1) return Math.round(n * 100);
            return Math.min(Math.round(n), 100);
        },

        checkMute: (btn) => {
            if (!btn) return false;
            const icon = btn.querySelector("svg use");
            if (icon) {
                const href = (icon.getAttribute('xlink:href') || '').toLowerCase();
                if (href.includes('off') || href.includes('mute')) return true;
            }
            return btn.className.includes('muted') || btn.className.includes('off');
        }
    };

    class YMController {
        constructor() {
            this.cache = {}; 
        }

        _findOne(key, root, selectors) {
            if (this.cache[key] && this.cache[key].isConnected) {
                return this.cache[key];
            }
            const el = Utils.find(root || document, selectors);
            if (el) this.cache[key] = el;
            return el;
        }

        _findBtnOne(key, root, selectors) {
            if (this.cache[key] && this.cache[key].isConnected) {
                return this.cache[key];
            }
            const el = Utils.findBtn(root || document, selectors);
            if (el) this.cache[key] = el;
            return el;
        }

        _getPlayer() {
            return this._findOne('root', document, DOM.SCOPE);
        }

        getFullState() {
            try {
                const root = this._getPlayer();
                if (!root) return { success: false, reason: 'BAR_NOT_FOUND' };

                const titleEl = Utils.find(root, DOM.Track.TITLE);
                const artistEl = Utils.find(root, DOM.Track.ARTIST);
                
                const likeBtn = this._findBtnOne('likeBtn', root, DOM.Controls.LIKE);
                const dislikeBtn = this._findBtnOne('dislikeBtn', root, DOM.Controls.DISLIKE);
                
                const pauseBtn = root.querySelector("button[class*='pause'], [data-test-id='PAUSE_BUTTON']");
                
                let cover = Utils.find(root, DOM.Track.COVER)?.src;
                if (cover) cover = cover.replace(/\d+x\d+/, '200x200');

                return {
                    success: true,
                    data: {
                        track: {
                            id: Utils.resolveTrackId(root, titleEl),
                            title: titleEl?.textContent?.trim() || "Unknown",
                            artist: artistEl?.textContent?.trim() || "Unknown",
                            cover: cover
                        },
                        state: {
                            playing: !!pauseBtn,
                            liked: Utils.isBtnActive(likeBtn),
                            disliked: Utils.isBtnActive(dislikeBtn)
                        },
                        progress: this._getProgressState(root),
                        volume: this._getVolumeState(root)
                    }
                };
            } catch (e) {
                return { success: false, error: e.toString() };
            }
        }

        playPause() {
            try {
                const root = this._getPlayer();
                if (!root) return { success: false };
                
                const btn = this._findBtnOne('playBtn', root, DOM.Controls.PLAY_PAUSE);
                if (btn) {
                    btn.click();
                    const wasPlaying = btn.className.includes('pause') || btn.getAttribute('data-test-id') === 'PAUSE_BUTTON';
                    return { success: true, is_playing: !wasPlaying };
                }
                return { success: false, error: "No control button" };
            } catch (e) { return { success: false, error: e.toString() }; }
        }

        next() { return this._clickSimple('nextBtn', DOM.Controls.NEXT, "skipped"); }
        prev() { return this._clickSimple('prevBtn', DOM.Controls.PREV, "prev_clicked"); }

        toggleLike() {
             const root = this._getPlayer();
             const btn = this._findBtnOne('likeBtn', root, DOM.Controls.LIKE);
             if (!btn) return { success: false };
             
             const wasLiked = Utils.isBtnActive(btn);
             btn.click();
             return { success: true, new_state: !wasLiked };
        }

        toggleDislike() {
             const root = this._getPlayer();
             const btn = this._findBtnOne('dislikeBtn', root, DOM.Controls.DISLIKE);
             if (!btn) return { success: false };
             
             const wasDisliked = Utils.isBtnActive(btn);
             btn.click();
             return { success: true, is_disliked: !wasDisliked };
        }

        changeVolume(action, value) {
            try {
                const currentVolInfo = this._getVolumeState(); 
                const currentVol = currentVolInfo.current / 100;
                const step = 0.05;

                switch(action) {
                    case 'UP': return this._setVolume(currentVol + step);
                    case 'DOWN': return this._setVolume(currentVol - step);
                    case 'SET': return this._setVolume(value / 100);
                    case 'MUTE': return this._toggleMute();
                    default: return { success: false, error: "Unknown action" };
                }
            } catch (e) { return { success: false, error: e.toString() }; }
        }

        _getProgressState(root) {
            let now = 0;
            let duration = 0;
            let progress = 0;
            try {
                if (window.externalAPI) {
                    const rawProgress = window.externalAPI.getProgress();
                    const rawDuration = window.externalAPI.getDuration();
                    
                    now = (typeof rawProgress === 'number' && !isNaN(rawProgress)) ? rawProgress : 0;
                    duration = (typeof rawDuration === 'number' && !isNaN(rawDuration)) ? rawDuration : 0;
                    progress = (duration > 0) ? (now / duration) : 0;
                } else {
                    const slider = Utils.find(root, DOM.Controls.TIMELINE);
                    if (slider) {
                        const val = parseFloat(slider.value) || 0;
                        const max = parseFloat(slider.max) || 0; 
                        
                        now = val; 
                        duration = max;
                        progress = (max > 0) ? (val / max) : 0;
                    } else {
                        const timeNowStr = Utils.find(root, DOM.Controls.TIME_NOW)?.textContent || "0:00";
                        const timeEndStr = Utils.find(root, DOM.Controls.TIME_END)?.textContent || "0:00";
                        
                        now = Utils.toSec(timeNowStr);
                        duration = Utils.toSec(timeEndStr);
                        progress = (duration > 0) ? (now / duration) : 0;
                    }
                }
            } catch(e) {}

            return { 
                now_sec: now,
                total_sec: duration,
                ratio: isNaN(progress) ? 0 : isFinite(progress) ? progress : 0
            };
        }

        _getVolumeState(root) {
            if (window.externalAPI && typeof window.externalAPI.getVolume === 'function') {
                try {
                    const apiVol = window.externalAPI.getVolume();
                    let isMuted = false;
                    if (typeof window.externalAPI.getMute === 'function') {
                        isMuted = window.externalAPI.getMute();
                    } else {
                        isMuted = Utils.checkMute(this._findBtnOne('muteBtn', root || document, DOM.Volume.MUTE_BTN));
                    }
                    return { current: Utils.toPercent(apiVol), is_muted: isMuted, method: 'api' };
                } catch (e) {}
            }

            const slider = this._findOne('volSlider', root || document, DOM.Volume.SLIDER);
            if (slider) {
                const val = parseFloat(slider.value);
                const max = parseFloat(slider.max) || 1;
                const ratio = (max <= 1) ? val : (val / max);
                const isMuted = Utils.checkMute(this._findBtnOne('muteBtn', root || document, DOM.Volume.MUTE_BTN));
                return { current: Utils.toPercent(ratio), is_muted: isMuted, method: 'dom' };
            }

            return { current: 0, is_muted: false, method: 'none' };
        }

        _setVolume(val) {
            const clamped = Math.max(0, Math.min(1, Math.round(val * 100) / 100));
            
            if (window.externalAPI && typeof window.externalAPI.setVolume === 'function') {
                try {
                    window.externalAPI.setVolume(clamped);
                    return { success: true, volume: Math.round(clamped * 100) };
                } catch (e) {}
            }

            const slider = Utils.find(document, DOM.Volume.SLIDER);
            if (slider) {
                const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value").set;
                if (setter) {
                setter.call(slider, clamped);
                slider.dispatchEvent(new Event('input', { bubbles: true }));
                slider.dispatchEvent(new Event('change', { bubbles: true }));
                return { success: true, volume: Math.round(clamped * 100) };
                }
                slider.value = clamped;
                slider.dispatchEvent(new Event('change', { bubbles: true }));
                return { success: true, volume: Math.round(clamped * 100) };
            }
            return { success: false, error: "Volume control unavailable" };
        }

        _toggleMute() {
            if (window.externalAPI && typeof window.externalAPI.toggleMute === 'function') {
                window.externalAPI.toggleMute();
                return { success: true };
            }
            const btn = this._findBtnOne('muteBtn', document, DOM.Volume.MUTE_BTN);
            if (btn) {
                btn.click();
                return { success: true };
            }
            return { success: false, error: "Mute button unavailable" };
        }

        _clickSimple(key, selectors, actionName) {
            try {
                const root = this._getPlayer();
                if (!root) return { success: false };
                const btn = this._findBtnOne(key, root, selectors);
                if (!btn || btn.disabled) return { success: false, reason: 'btn_not_found_or_disabled' };
                
                btn.click();
                return { success: true, action: actionName };
            } catch (e) { return { success: false, error: e.toString() }; }
        }
        
        deepDiff(obj1, obj2) {
            if (obj1 === obj2) return undefined;
            if (typeof obj1 !== typeof obj2 || obj1 === null || obj2 === null) return obj2;
            
            if (Array.isArray(obj1)) {
                if (obj1.length !== obj2.length) return obj2;
                for (let i = 0; i < obj1.length; i++) {
                    if (JSON.stringify(obj1[i]) !== JSON.stringify(obj2[i])) return obj2;
                }
                return undefined;
            }

            if (typeof obj1 === 'object') {
                const diff = {};
                let hasChange = false;
                
                for (let key in obj2) {
                    if (!(key in obj1)) {
                        diff[key] = obj2[key];
                        hasChange = true;
                    } else {
                        const d = this.deepDiff(obj1[key], obj2[key]);
                        if (d !== undefined) {
                            diff[key] = d;
                            hasChange = true;
                        }
                    }
                }
                
                return hasChange ? diff : undefined;
            }
            
            return obj2;
        }

        startObservation() {
            if (this.observing) return;
            this.observing = true;
            this.lastState = null;
            
            const loop = () => {
                if (!this.observing) return;
                
                const raw = this.getFullState();
                if (raw && raw.success) {
                    const currentState = raw.data;
                    
                    if (!this.lastState) {
                        this._notify('FULL_STATE', currentState);
                        this.lastState = currentState;
                    } else {
                        const delta = this.deepDiff(this.lastState, currentState);
                        if (delta) {
                            this.lastState = currentState;
                            this._notify('DELTA', delta);
                        }
                    }
                }
                
                setTimeout(loop, 100);
            };
            
            loop();
        }
        
        stopObservation() {
            this.observing = false;
        }

        forceSync() {
            this.lastState = null;
            
            if (!this.observing) {
                const raw = this.getFullState();
                if (raw && raw.success) {
                    this._notify('FULL_STATE', raw.data);
                    this.lastState = raw.data;
                }
            }
        }

        _notify(type, payload) {
            const msg = JSON.stringify({ type: type, payload: payload });
            
            if (window.sdNotify) {
                window.sdNotify(msg);
            } else {
                // pass
            }
        }
    }

    const ctrl = new YMController();
    window._PyYMController = ctrl;
    
    ctrl.startObservation();

    return true;
})();