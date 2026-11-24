//! Particles 2's fields in a scene file: an emitter written before them
//! still loads as it was, one using them round-trips, and a plain one
//! writes none of them.

use voltra_assets::AssetPath;
use voltra_render::glam::Vec2;

use super::*;
use crate::curve::{Curve, CurveKey};
use crate::gradient::Gradient;

/// An emitter as stage 51 wrote it.
const BEFORE: &str = "(emitting:true,rate:20.0,max_particles:256,lifetime:(0.8,1.2),\
speed:(1.0,2.0),direction:1.5707964,spread:0.5,acceleration:(0.0,-2.0),size:(0.2,0.0),\
color_start:(1.0,1.0,1.0,1.0),color_end:(1.0,1.0,1.0,0.0),texture:None,sort_order:0,\
world_space:true,one_shot:false)";

#[test]
fn an_emitter_written_before_particles_2_loads_as_it_was() {
    let read: ParticleEmitter = ron::from_str(BEFORE).expect("an old emitter reads");
    assert_eq!(read, ParticleEmitter::default());
}

#[test]
fn a_plain_emitter_writes_none_of_the_new_fields() {
    let text = ron::to_string(&ParticleEmitter::default()).expect("writes");
    for field in [
        "shape",
        "curve",
        "gradient",
        "rotation",
        "angular_velocity",
        "flipbook",
    ] {
        assert!(!text.contains(field), "{field} in {text}");
    }
}

#[test]
fn every_new_field_round_trips() {
    let emitter = ParticleEmitter {
        shape: EmitShape::Ring {
            inner: 0.5,
            outer: 1.0,
        },
        size_curve: Some(Curve::linear(1.0, 0.0)),
        speed_curve: Some(Curve::new(vec![CurveKey::new(0.5, 2.0)])),
        alpha_curve: Some(Curve::constant(0.5)),
        gradient: Some(Gradient::linear([1.0, 0.5, 0.0, 1.0], [0.2, 0.2, 0.2, 0.0])),
        rotation: (0.0, 1.0),
        angular_velocity: (-2.0, 2.0),
        flipbook: Some(Flipbook {
            frames: vec![3, 1, 2],
            timing: FlipbookTiming::PerSecond { fps: 12.0 },
            ..Flipbook::new(AssetPath::new("fx/smoke.atlas.ron").expect("a path"))
        }),
        ..ParticleEmitter::default()
    };
    let text = ron::to_string(&emitter).expect("writes");
    let read: ParticleEmitter = ron::from_str(&text).expect("reads");
    assert_eq!(read, emitter);
    let boxed = ParticleEmitter {
        shape: EmitShape::Box {
            half_extents: Vec2::new(1.0, 2.0),
        },
        ..ParticleEmitter::default()
    };
    let text = ron::to_string(&boxed).expect("writes");
    assert_eq!(
        ron::from_str::<ParticleEmitter>(&text).expect("reads"),
        boxed
    );
}
