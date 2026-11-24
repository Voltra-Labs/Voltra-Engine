# 6. Audio

- **`AudioSource`** on an entity plays a clip (WAV, OGG Vorbis, FLAC,
  MP3) — looped or once, at a volume and pitch, panned and attenuated by
  its distance from the **`AudioListener`** (usually on the camera). Drag
  a sound file into the viewport to make one; the inspector's Play button
  auditions it.
- **One-shots** from a tick, with no entity: `tick.audio.play(handle)` —
  a coin, a jump. Take the handle from a source already in the scene so
  the path stays in the scene file.
- **Buses** group sounds (`music`, `effects`) so a settings slider can turn
  one down: `tick.audio.set_bus_volume(bus, 0.5, Duration::from_millis(300))`,
  faded over the given time.

In a browser, sound waits for the first key, click or touch: browsers keep
audio off until the player interacts, and the engine resumes it then.
