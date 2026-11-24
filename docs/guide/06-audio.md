# 6. Audio

- **`AudioSource`** on an entity plays a clip (WAV, OGG Vorbis, FLAC,
  MP3) — looped or once, at a volume and pitch, panned and attenuated by
  its distance from the **`AudioListener`** (usually on the camera). Drag
  a sound file into the viewport to make one; the inspector's Play button
  auditions it.
- **One-shots** from a tick, with no entity: `tick.audio.play(handle)` —
  a coin, a jump. Take the handle from a source already in the scene
  (`AudioSource::clip_handle`) so the path stays in the scene file.
- **Buses** group sounds (`music`, `sfx`, `ui`) so a settings slider can
  turn one down: `tick.audio.set_bus_volume(bus, 0.5,
  Duration::from_millis(300))`, faded over the given time;
  `tick.audio.bus("music")` names one.

In a browser, sound waits for the first key, click or touch: browsers keep
audio off until the player interacts, and the engine resumes it then.

## Music

A sound file is decoded whole when it loads — right for a coin, megabytes
a minute for music. A long track gets a settings file of its own, a
`*.clip.ron`, named wherever a sound is named (an `AudioSource`'s clip, a
drop on the viewport):

```ron
(version: 1, file: "audio/theme.ogg", stream: true, loop_start: 4.0, loop_end: Some(62.5))
```

`stream: true` decodes it while it plays, a second ahead, instead of
holding it all in memory. `loop_start` and `loop_end` (seconds; `None` is
the end of the file) make a looping track play its intro once, then loop
the part between — seamlessly, with or without streaming. Editing the
`.ogg` or the `.clip.ron` reloads it.

`tick.audio.play_music(handle, Duration::from_secs(2))` makes a clip the
music: looping, on the `music` bus, cross-fading from the last track over
the time given, at equal power so the mix does not dip in the middle.
Naming the track already playing changes nothing, so a tick may name its
area's music every frame. `tick.audio.stop_music(fade)` fades it out.

## Bus effects

The project's buses are in `assets/default.buses.ron` (Window ▸ Project
Settings ▸ Audio buses edits it; it is hot reloaded): each bus's volume and
the effects it runs, first to last, before its volume. Every bus feeds the
master.

```ron
(version: 1, buses: [
    (name: "music", volume: 0.8, effects: [Duck(by: "dialogue", depth: 0.7)]),
    (name: "sfx", effects: [Reverb(room_size: 0.6, mix: 0.25)]),
    (name: "dialogue"),
])
```

The effects are `LowPass(cutoff, q)`, `HighPass(cutoff, q)`,
`Delay(time, feedback, mix)`, `Reverb(room_size, damping, width, mix)` and
`Duck(by, depth, threshold, attack, release)` — the last pulls its bus
down while another bus sounds, the music under a voice line. Times are in
seconds, levels linear; a filter's `cutoff`, a delay's `time` and a duck's
`by` must be given, and the rest have defaults.

A tick moves an effect's numbers as it moves a volume — a low-pass opening
as the player surfaces from water:

```rust
use voltra_core::effect::Param;

let music = tick.audio.bus("music");
// With `effects: [LowPass(cutoff: 600.0)]` on the bus: its first effect
// (index 0) opens to 18 kHz over a second.
tick.audio.set_bus_param(music, 0, Param::Cutoff, 18_000.0, Duration::from_secs(1));
```

It answers `false` for an effect or a parameter that is not there.
`tick.audio.set_bus_effects(bus, effects)` replaces a bus's chain.
