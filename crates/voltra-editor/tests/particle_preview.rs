//! A particle emitter's curve edited through the bridge: the live preview
//! (stage 100) changes, and the edit is one undo entry.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test particle_preview -- --ignored

mod common;

use std::time::Duration;

use common::{call, launch, project_with, wait_for};
use voltra_bridge::{BridgeClient, Command};

/// An emitter of still particles over a box round the origin, its size
/// curve a constant `size` — the field the test edits.
fn emitter(size: f32) -> String {
    format!(
        "(emitting:true,rate:400.0,max_particles:400,lifetime:(30.0,30.0),speed:(0.0,0.0),\
direction:0.0,spread:0.0,acceleration:(0.0,0.0),size:(0.1,0.1),\
size_curve:Some([(t:0.0,value:{size:?})]),shape:Box(half_extents:(3.0,2.0)),\
color_start:(1.0,1.0,1.0,1.0),color_end:(1.0,1.0,1.0,1.0),texture:None,sort_order:5,\
world_space:true,one_shot:false)"
    )
}

/// The viewport, decoded.
fn screenshot(client: &mut BridgeClient) -> image::RgbaImage {
    use base64::Engine;
    let answer = call(client, Command::Screenshot);
    let png = base64::engine::general_purpose::STANDARD
        .decode(answer["png_base64"].as_str().expect("a png"))
        .expect("base64");
    image::load_from_memory(&png).expect("a png").to_rgba8()
}

/// Pixels nearly white: where particles are.
fn white(shot: &image::RgbaImage) -> usize {
    shot.pixels()
        .filter(|p| p.0[..3].iter().all(|&c| c > 230))
        .count()
}

/// The preview fills in over a second: the shot after it has.
fn settled(client: &mut BridgeClient) -> usize {
    std::thread::sleep(Duration::from_millis(1500));
    white(&screenshot(client))
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn a_curve_edited_through_the_bridge_changes_the_preview_as_one_undo_entry() {
    let scene = format!(
        r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-000000000002", components: {{"Name": "Fx", "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)), "ParticleEmitter": {}}})])"#,
        emitter(0.5)
    );
    let assets = project_with("particles", &scene);
    let (_editor, mut client) = launch(&assets);
    wait_for(&mut client, "Fx");
    // Selected and not playing: the preview runs.
    call(
        &mut client,
        Command::Select {
            entities: vec!["Fx".to_owned()],
        },
    );
    let status = call(&mut client, Command::Status);
    assert_eq!(status["play"], "Editing");
    assert!(status["undo"].is_null(), "nothing done yet: {status}");
    let small = settled(&mut client);
    assert!(
        small > 50,
        "the preview shows particles: {small} white pixels"
    );

    // The size curve three times as big, through the bridge.
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Fx".to_owned(),
            component: "ParticleEmitter".to_owned(),
            ron: emitter(1.5),
        },
    );
    let big = settled(&mut client);
    assert!(
        big > small * 3,
        "a bigger size curve covers more: {small} then {big}"
    );

    // One entry: undone once, nothing is left to undo.
    let status = call(&mut client, Command::Status);
    assert!(status["undo"].is_string(), "the edit is an entry: {status}");
    call(&mut client, Command::Undo);
    let status = call(&mut client, Command::Status);
    assert!(status["undo"].is_null(), "one entry, not more: {status}");
    let read = call(&mut client, Command::Read);
    assert!(
        read.to_string().contains("value:0.5"),
        "the curve is back: {read}"
    );
    let back = settled(&mut client);
    assert!(
        back < big / 2,
        "the undone curve draws small again: {back} after {big}"
    );
}
