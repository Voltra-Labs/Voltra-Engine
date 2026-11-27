//! Property animation: clips sampled, poses mixed, and players advanced —
//! one clip, a crossfade, and several clips blended at once.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::{resolve_parents, set_parent};
use voltra_scene::keyframes::{
    self, Ease, Key, LayerBlend, PlayerLayer, Property, SyncedClips, Track, Value,
};
use voltra_scene::{
    AnimationClip, AnimationClips, AnimationPlayer, Name, SceneId, Sprite2D, Transform2D,
};

use super::{Bench, Outcome};
use crate::harness::{iter, measure};
use voltra_testkit::Scratch;

/// Named children each clip animates: a small rig, a limb or two each.
const TARGETS: usize = 8;

/// Players advanced per frame: a crowd of animated characters.
const PLAYERS: usize = 100;

/// The clips the players blend: an idle, a walk and a run, say.
const CLIPS: [&str; 3] = ["a.anim.ron", "b.anim.ron", "c.anim.ron"];

/// A frame's step.
const FRAME: f32 = 1.0 / 60.0;

pub(super) fn benches() -> Vec<Bench> {
    let tracks = TARGETS * 3;
    vec![
        // Under two microseconds each: steadied (`Config::steady`).
        Bench::new(
            format!("keyframes/sample/{tracks}-tracks"),
            30.0,
            |config| {
                let one = clip(0);
                measure(&config.steady(), 1, iter(|| one.sample(0.37))).into()
            },
        ),
        Bench::new(format!("keyframes/blend/3x{tracks}"), 30.0, |config| {
            let clips: Vec<AnimationClip> = (0..CLIPS.len()).map(clip).collect();
            let poses: Vec<_> = clips.iter().map(|c| c.sample(0.37)).collect();
            measure(
                &config.steady(),
                1,
                iter(|| keyframes::blend(&[(&poses[0], 0.2), (&poses[1], 0.5), (&poses[2], 0.3)])),
            )
            .into()
        }),
        Bench::new(
            format!("keyframes/advance/{PLAYERS}-playing"),
            30.0,
            |config| {
                let Some((_root, mut world, mut clips, _)) = crowd() else {
                    return unwritable();
                };
                measure(
                    config,
                    1,
                    iter(|| keyframes::advance(&mut world, &mut clips, FRAME)),
                )
                .into()
            },
        ),
        Bench::new(
            format!("keyframes/advance/{PLAYERS}-blending-{}", CLIPS.len()),
            30.0,
            |config| {
                let Some((_root, mut world, mut clips, players)) = crowd() else {
                    return unwritable();
                };
                for &player in &players {
                    if let Some(player) = world.get_mut::<AnimationPlayer>(player) {
                        // A blend tree's three children, all weighing in.
                        player.play_synced(SyncedClips::new(
                            CLIPS.iter().zip([0.2, 0.5, 0.3]).map(|(c, w)| (path(c), w)),
                        ));
                    }
                }
                measure(
                    config,
                    1,
                    iter(|| keyframes::advance(&mut world, &mut clips, FRAME)),
                )
                .into()
            },
        ),
        Bench::new(
            format!("keyframes/advance/{PLAYERS}-layered"),
            30.0,
            |config| {
                let Some((_root, mut world, mut clips, players)) = crowd() else {
                    return unwritable();
                };
                for &player in &players {
                    if let Some(player) = world.get_mut::<AnimationPlayer>(player) {
                        // An upper body masked to half the parts, and an
                        // additive breath over everything.
                        let mut upper = PlayerLayer::new(LayerBlend::Override);
                        upper.player.play(path(CLIPS[1]));
                        upper.mask = (0..TARGETS / 2).map(|t| format!("part{t}")).collect();
                        upper.weight = 0.7;
                        let mut breath = PlayerLayer::new(LayerBlend::Additive);
                        breath.player.play(path(CLIPS[2]));
                        breath.weight = 0.5;
                        player.layers_mut().extend([upper, breath]);
                    }
                }
                measure(
                    config,
                    1,
                    iter(|| keyframes::advance(&mut world, &mut clips, FRAME)),
                )
                .into()
            },
        ),
        Bench::new(
            format!("keyframes/advance/{PLAYERS}-crossfading"),
            30.0,
            |config| {
                let Some((_root, mut world, mut clips, players)) = crowd() else {
                    return unwritable();
                };
                keyframes::advance(&mut world, &mut clips, FRAME);
                for &player in &players {
                    if let Some(player) = world.get_mut::<AnimationPlayer>(player) {
                        // Long enough that every timed frame is mid-fade.
                        player.cross_fade(path(CLIPS[1]), 1e9);
                    }
                }
                measure(
                    config,
                    1,
                    iter(|| keyframes::advance(&mut world, &mut clips, FRAME)),
                )
                .into()
            },
        ),
    ]
}

fn unwritable() -> Outcome {
    Outcome::Skipped("the generated clips would not write".to_owned())
}

fn path(name: &str) -> AssetPath {
    AssetPath::new(name).expect("a literal relative path")
}

/// Clip `n` of the set: every target's translation, rotation and colour,
/// keyed four times over a looping second, each clip its own values.
fn clip(n: usize) -> AnimationClip {
    let step = n as f32 + 1.0;
    let mut tracks = Vec::new();
    for t in 0..TARGETS {
        let target = format!("part{t}");
        let keyed = |property, value: &dyn Fn(f32) -> Value| {
            let mut track = Track::new(target.clone(), property);
            track.keys = (0..4)
                .map(|k| {
                    let mut key = Key::new(k as f32 / 3.0, value(k as f32 * step));
                    key.ease = Ease::EaseInOut;
                    key
                })
                .collect();
            track
        };
        tracks.push(keyed(Property::Translation, &|v| {
            Value::Vec2(Vec2::new(v, -v))
        }));
        tracks.push(keyed(Property::Rotation, &|v| Value::Float(v * 0.1)));
        tracks.push(keyed(Property::SpriteColor, &|v| {
            Value::Color([v * 0.1, 0.5, 1.0 - v * 0.1, 1.0])
        }));
    }
    AnimationClip {
        length: 1.0,
        looping: true,
        tracks,
    }
}

/// [`PLAYERS`] players, each with [`TARGETS`] named children carrying a
/// transform and a sprite, playing the first clip; the store holds every
/// clip of the set, written to a scratch root (removed when the answer's
/// first element drops). Answers the players too.
fn crowd() -> Option<(Scratch, World, AnimationClips, Vec<Entity>)> {
    let root = voltra_testkit::scratch_root();
    let clips = AnimationClips::new(&root);
    for (n, name) in CLIPS.iter().enumerate() {
        clips.write(&path(name), &clip(n)).ok()?;
    }
    let mut world = World::new();
    let mut players = Vec::with_capacity(PLAYERS);
    for _ in 0..PLAYERS {
        let player = world.spawn();
        world.insert(player, SceneId::new());
        world.insert(player, Transform2D::default());
        world.insert(player, AnimationPlayer::new(path(CLIPS[0])));
        for t in 0..TARGETS {
            let part = world.spawn();
            world.insert(part, SceneId::new());
            world.insert(part, Name::new(format!("part{t}")));
            world.insert(part, Transform2D::default());
            world.insert(part, Sprite2D::default());
            set_parent(&mut world, part, player).ok()?;
        }
        players.push(player);
    }
    resolve_parents(&mut world);
    Some((root, world, clips, players))
}
