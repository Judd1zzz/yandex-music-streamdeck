export const SONATA_HTML = `<!DOCTYPE html><html><body>
  <div data-test-id="PLAYERBAR_DESKTOP" class="PlayerBarDesktopWithBackgroundProgressBar_root__abc">
    <div class="PlayerBarDesktopWithBackgroundProgressBar_sonata__def">
      <a data-test-id="TRACK_TITLE" href="/track/12345" class="PlayerBarTitle_title__t">Track Title</a>
      <span data-test-id="SEPARATED_ARTIST_TITLE" class="PlayerBarTitle_artist__a">Artist Name</span>
      <div class="PlayerBarDesktopWithBackgroundProgressBar_cover__c">
        <img data-test-id="ENTITY_COVER_IMAGE" src="https://avatars.yandex.net/get-music-content/abc/100x100">
      </div>
      <button data-test-id="PREVIOUS_TRACK_BUTTON" class="BaseSonataControlsDesktop_sonataButton__b" aria-label="Previous">prev</button>
      <button data-test-id="PAUSE_BUTTON" class="BaseSonataControlsDesktop_sonataButton__b ControlButton_pause">pause</button>
      <button data-test-id="NEXT_TRACK_BUTTON" class="BaseSonataControlsDesktop_sonataButton__b" aria-label="Next">next</button>
      <button data-test-id="DISLIKE_BUTTON" aria-pressed="false">dislike</button>
      <button data-test-id="LIKE_BUTTON" aria-pressed="false">like</button>
      <input type="range" data-test-id="TIMECODE_SLIDER" min="0" max="240" value="60">
      <span data-test-id="TIMECODE_TIME_START">1:00</span>
      <span data-test-id="TIMECODE_TIME_END">4:00</span>
      <div class="ChangeVolume_root__v">
        <input type="range" data-test-id="VOLUME_SLIDER" class="ChangeVolume_slider__s" min="0" max="1" step="0.01" value="0.5">
        <button data-test-id="VOLUME_BUTTON" class="ChangeVolume_button__m" aria-label="Mute">mute</button>
      </div>
    </div>
  </div>
</body></html>`;

export const EMPTY_HTML = `<!DOCTYPE html><html><body></body></html>`;
