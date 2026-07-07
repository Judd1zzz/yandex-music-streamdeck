export const VIBE_HTML = `<!DOCTYPE html><html><body>
  <div class="VibePage_entityMeta__m">
    <a data-test-id="SEPARATED_ARTIST_TITLE" href="/artist/111" aria-label="Artist Madonna">Madonna</a>
  </div>
  <section data-test-id="VIBE_PLAYERBAR" aria-label="Player" class="VibePlayerBar_root__r">
    <div data-test-id="VIBE_ALBUM_COVER">
      <a href="/album?albumId=1"><img class="AlbumCover_cover__i" src="https://avatars.yandex.net/get-music-content/2383988/x/400x400"></a>
    </div>
    <div class="ChangeVolume_root__v">
      <div class="ChangeVolume_sliderContainer__c"><div class="ChangeVolume_wrapperSlider__w"><input max="1" step="0.01" aria-label="Manage volume" data-test-id="CHANGE_VOLUME_SLIDER" type="range" value="0.5"></div></div>
      <button type="button" aria-label="Turn off sound" data-test-id="CHANGE_VOLUME_BUTTON" class="ChangeVolume_button__b"><span><svg><use xlink:href="/icons/sprite.svg#volume_xs"></use></svg></span></button>
    </div>
    <button type="button" aria-label="I don't like it" aria-pressed="false" data-test-id="DISLIKE_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#dislike_xs"></use></svg></span></button>
    <div data-test-id="VIBE_PLAYERBAR_TIMECODE_SLIDER">
      <div data-test-id="VIBE_PLAYERBAR_TRACK_NAME"><div class="VibePlayerbarMeta_trackNameText__t" aria-hidden="true">Faded</div><div class="VibePlayerbarMeta_trackNameText__t">Faded</div></div>
      <span aria-hidden="true" data-test-id="VIBE_PLAYERBAR_TIMECODE">00:38 / 02:59</span>
      <input max="179" aria-label="Manage time code" class="VibePlayerbarMeta_slider__s" type="range" value="38">
    </div>
    <button type="button" aria-label="Like" aria-pressed="false" data-test-id="LIKE_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#like_xs"></use></svg></span></button>
    <div><button type="button" data-test-id="VIBE_CONTEXT_MENU_BUTTON" aria-label="Context menu"><span><svg><use xlink:href="/icons/sprite.svg#more_xs"></use></svg></span></button></div>
    <div class="VibePlayerControls_root__c">
      <button type="button" aria-label="Previous song" data-test-id="PREVIOUS_TRACK_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#previous_xs"></use></svg></span></button>
      <button type="button" aria-label="Pause" data-test-id="PAUSE_BUTTON" class="VibePlayerControls_playButton__p VibePlayerControls_playButton_playing__q"><span><svg><use xlink:href="/icons/sprite.svg#pause"></use></svg></span></button>
      <button type="button" aria-label="Next song" data-test-id="NEXT_TRACK_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#next_xs"></use></svg></span></button>
    </div>
  </section>
</body></html>`;

export const SONATA_PLUS_HIDDEN_VIBE_HTML = `<!DOCTYPE html><html><body>
  <div data-test-id="PLAYERBAR_DESKTOP" class="PlayerBarDesktopWithBackgroundProgressBar_root__s">
    <a data-test-id="TRACK_TITLE" href="/track/999">Sonata Track</a>
    <button data-test-id="PAUSE_BUTTON" class="ControlButton_pause">pause</button>
  </div>
  <section data-test-id="VIBE_PLAYERBAR" class="VibePlayerBar_root__r">
    <div class="VibePlayerControls_root__c">
      <button data-test-id="PAUSE_BUTTON" class="VibePlayerControls_playButton__p">pause</button>
    </div>
    <div data-test-id="VIBE_PLAYERBAR_TRACK_NAME"><div class="VibePlayerbarMeta_trackNameText__t">Vibe Track</div></div>
  </section>
</body></html>`;

// «Моя Волна» на ПАУЗЕ: центральный дисплей возвращается к "My Vibe" (раздельных
// SEPARATED_ARTIST_TITLE нет), а плеербар схлопывается в одну строку «Артист — Название».
// title/artist должны остаться стабильными (split по em-dash U+2014).
export const VIBE_MY_VIBE_PAUSED_HTML = `<!DOCTYPE html><html><body>
  <div class="VibePage_entityMeta__m">
    <span data-test-id="VIBE_DYNAMIC_TITLE_VIBE" class="_MWOVuZRvUQdXKTMcOPx">My Vibe</span>
  </div>
  <section data-test-id="VIBE_PLAYERBAR" aria-label="Player" class="VibePlayerBar_root__r">
    <div data-test-id="VIBE_ALBUM_COVER">
      <a href="/album?albumId=1"><img class="AlbumCover_cover__i" src="https://avatars.yandex.net/get-music-content/2383988/x/400x400"></a>
    </div>
    <div class="ChangeVolume_root__v">
      <div class="ChangeVolume_sliderContainer__c"><div class="ChangeVolume_wrapperSlider__w"><input max="1" step="0.01" aria-label="Manage volume" data-test-id="CHANGE_VOLUME_SLIDER" type="range" value="0.5"></div></div>
      <button type="button" aria-label="Turn off sound" data-test-id="CHANGE_VOLUME_BUTTON" class="ChangeVolume_button__b"><span><svg><use xlink:href="/icons/sprite.svg#volume_xs"></use></svg></span></button>
    </div>
    <button type="button" aria-label="I don't like it" aria-pressed="false" data-test-id="DISLIKE_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#dislike_xs"></use></svg></span></button>
    <div data-test-id="VIBE_PLAYERBAR_TIMECODE_SLIDER">
      <div data-test-id="VIBE_PLAYERBAR_TRACK_NAME"><div class="VibePlayerbarMeta_trackNameText__t" aria-hidden="true">Madonna  —  Faded</div><div class="VibePlayerbarMeta_trackNameText__t">Madonna  —  Faded</div></div>
      <span aria-hidden="true" data-test-id="VIBE_PLAYERBAR_TIMECODE">00:38 / 02:59</span>
      <input max="179" aria-label="Manage time code" class="VibePlayerbarMeta_slider__s" type="range" value="38">
    </div>
    <button type="button" aria-label="Like" aria-pressed="false" data-test-id="LIKE_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#like_xs"></use></svg></span></button>
    <div><button type="button" data-test-id="VIBE_CONTEXT_MENU_BUTTON" aria-label="Context menu"><span><svg><use xlink:href="/icons/sprite.svg#more_xs"></use></svg></span></button></div>
    <div class="VibePlayerControls_root__c">
      <button type="button" aria-label="Previous song" data-test-id="PREVIOUS_TRACK_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#previous_xs"></use></svg></span></button>
      <button type="button" aria-label="Play" data-test-id="PLAY_BUTTON" class="VibePlayerControls_playButton__p"><span><svg><use xlink:href="/icons/sprite.svg#play"></use></svg></span></button>
      <button type="button" aria-label="Next song" data-test-id="NEXT_TRACK_BUTTON"><span><svg><use xlink:href="/icons/sprite.svg#next_xs"></use></svg></span></button>
    </div>
  </section>
</body></html>`;

// Vibe-бар, у которого выжили только data-test-id (все хешированные классы сменились):
// проверяет test-id-слои гейта, тайтла, обложки и комбинированного таймкода.
export const VIBE_TESTID_ONLY_HTML = `<!DOCTYPE html><html><body>
  <section data-test-id="VIBE_PLAYERBAR" aria-label="Player">
    <div data-test-id="VIBE_ALBUM_COVER">
      <a href="/album?albumId=1"><img src="https://avatars.yandex.net/get-music-content/2383988/x/400x400"></a>
    </div>
    <button type="button" aria-label="I don't like it" aria-pressed="false" data-test-id="DISLIKE_BUTTON"><span></span></button>
    <div data-test-id="VIBE_PLAYERBAR_TRACK_NAME"><div>Faded</div></div>
    <span aria-hidden="true" data-test-id="VIBE_PLAYERBAR_TIMECODE">00:38 / 02:59</span>
    <button type="button" aria-label="Like" aria-pressed="false" data-test-id="LIKE_BUTTON"><span></span></button>
    <button type="button" aria-label="Previous song" data-test-id="PREVIOUS_TRACK_BUTTON"><span></span></button>
    <button type="button" aria-label="Pause" data-test-id="PAUSE_BUTTON"><span></span></button>
    <button type="button" aria-label="Next song" data-test-id="NEXT_TRACK_BUTTON"><span></span></button>
  </section>
</body></html>`;

export const VIBE_TESTID_ONLY_WITH_SLIDER_HTML = `<!DOCTYPE html><html><body>
  <section data-test-id="VIBE_PLAYERBAR" aria-label="Player">
    <div data-test-id="VIBE_PLAYERBAR_TRACK_NAME"><div>Faded</div></div>
    <span aria-hidden="true" data-test-id="VIBE_PLAYERBAR_TIMECODE">00:38 / 02:59</span>
    <input data-test-id="TIMECODE_SLIDER" aria-label="Manage time code" type="range" max="200" value="50">
    <button type="button" aria-label="Pause" data-test-id="PAUSE_BUTTON"><span></span></button>
  </section>
</body></html>`;
