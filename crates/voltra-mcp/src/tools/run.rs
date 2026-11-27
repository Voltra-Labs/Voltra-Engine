//! Running a scene headless for a while, and reporting what came of it.

use serde::Deserialize;
use serde_json::{json, Value};
use voltra_core::{App, Headless, ScreenshotError, WindowConfig};
use voltra_render::glam::Vec2;
use voltra_scene::{Name, SceneId, Transform};

use super::ToolError;
use crate::Project;

/// The most frames one run may ask for: a minute and a half at 60 Hz.
pub const MAX_FRAMES: u32 = 6000;
/// The most log lines a run hands back; the latest are kept.
const MAX_LOG_LINES: usize = 200;
/// The most entities a run with no report list sums up.
const MAX_SUMMARY: usize = 100;
/// The widest and tallest screenshot a run takes, in pixels.
const MAX_SHOT_SIDE: u32 = 2048;

/// An action held down over frames of the run.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct HeldAction {
    /// The input map's action ("jump", "move_right").
    pub action: String,
    /// The frame it goes down on, from 0.
    pub press_at: u32,
    /// The frame it is let go on; absent holds it to the end.
    #[serde(default)]
    pub release_at: Option<u32>,
}

/// What to run, and what to bring back.
#[derive(Debug, Clone)]
pub struct RunRequest {
    /// The scene to run, as an asset path.
    pub scene: String,
    /// How many frames to run.
    pub frames: u32,
    /// Seconds each frame advances.
    pub delta: f32,
    /// Actions pressed and let go on their frames.
    pub inputs: Vec<HeldAction>,
    /// Entities to report in full, by name or reference; empty sums up where each named entity ended up.
    pub report: Vec<String>,
    /// Width and height in pixels of the screenshot to take at the end, if any.
    pub screenshot: Option<(u32, u32)>,
    /// Whether to time the run's frames, scope by scope.
    pub profile: bool,
}

/// What came of a run: the report, and the screenshot as PNG bytes.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The run as JSON: scene, frames, entities, unbound actions, log, screenshot status and profile.
    pub report: Value,
    /// The screenshot, PNG encoded, when one was asked for.
    pub png: Option<Vec<u8>>,
}

/// Runs `request.scene` headless: `frames` frames of `delta` seconds, the
/// actions pressed and let go on their frames, through the game loop the
/// player runs. Answers the entities asked for — every component, as RON —
/// or, with none asked for, where each named entity ended up; the log; and
/// a screenshot through the scene's camera when one is asked for.
///
/// Blocking: call it off the async runtime.
pub fn run_scene(project: &Project, request: &RunRequest) -> Result<RunOutcome, ToolError> {
    let (asset, _) = project
        .resolve(&request.scene)
        .map_err(ToolError::from_cause)?;
    if request.frames > MAX_FRAMES {
        return Err(ToolError(format!("at most {MAX_FRAMES} frames a run")));
    }
    let delta = if request.delta.is_finite() && request.delta > 0.0 {
        request.delta
    } else {
        1.0 / 60.0
    };
    // A screen the HUD is laid out on, as a player's window is one: its
    // buttons take their faces and the controls their input. Without one a
    // shot drew every button in its sprite's own colour. The shot's size
    // when one is asked for, so the HUD is laid out where it is drawn;
    // else the window the app is made with.
    let window = WindowConfig::default();
    let (width, height) = request
        .screenshot
        .map_or((window.width, window.height), |(width, height)| {
            shot_size(width, height)
        });
    let app = App::new(window).with_asset_root(project.assets());
    let mut session = Headless::open(app, &asset)
        .map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?
        .with_screen(Vec2::new(width as f32, height as f32));

    // This blocking thread's recorder, emptied: a pool thread may have
    // profiled a run before.
    if request.profile {
        voltra_profile::clear();
        voltra_profile::set_enabled(true);
    }
    let mut unknown = Vec::new();
    for frame in 0..request.frames {
        for held in &request.inputs {
            if held.press_at == frame && !session.press(&held.action) {
                unknown.push(held.action.clone());
            }
            if held.release_at == Some(frame) {
                session.release(&held.action);
            }
        }
        session.step(1, delta);
    }
    let profile = if request.profile {
        // The last frame closes here, not at a next step that never comes.
        voltra_profile::end_frame();
        let summary = voltra_profile::summary(request.frames as usize);
        voltra_profile::set_enabled(false);
        voltra_profile::clear();
        voltra_bridge::profile::to_json(&summary)
    } else {
        Value::Null
    };

    let entities = if request.report.is_empty() {
        summary(session.world())
    } else {
        let mut asked = Vec::new();
        for reference in &request.report {
            let entity = voltra_scene::edit::find(session.world(), reference)
                .map_err(ToolError::from_cause)?;
            let id = session.world().get::<SceneId>(entity).copied();
            let ron = id
                .map(|id| session.entity_ron(id))
                .transpose()
                .map_err(ToolError::from_cause)?;
            asked.push(json!({ "entity": reference, "ron": ron }));
        }
        Value::Array(asked)
    };

    let mut log: Vec<String> = session
        .take_logs()
        .into_iter()
        .map(|line| format!("{} {}: {}", line.level, line.target, line.message))
        .collect();
    if log.len() > MAX_LOG_LINES {
        log.drain(..log.len() - MAX_LOG_LINES);
    }

    let (png, screenshot) = match request.screenshot {
        None => (None, Value::Null),
        Some((width, height)) => {
            let (width, height) = shot_size(width, height);
            match session.screenshot(width, height) {
                Ok(shot) => match shot.png_bytes() {
                    Ok(bytes) => (
                        Some(bytes),
                        json!({ "width": shot.width, "height": shot.height }),
                    ),
                    Err(e) => (None, json!(e.to_string())),
                },
                Err(ScreenshotError::NoDevice) => (None, json!("no GPU adapter on this machine")),
                Err(e) => (None, json!(e.to_string())),
            }
        }
    };

    Ok(RunOutcome {
        report: json!({
            "scene": asset.as_str(),
            "frames": session.frames(),
            "seconds": session.frames() as f32 * delta,
            "entities": entities,
            "unbound_actions": unknown,
            "log": log,
            "screenshot": screenshot,
            "profile": profile,
        }),
        png,
    })
}

/// Where each named entity is: what a run with nothing asked for answers.
fn summary(world: &voltra_ecs::World) -> Value {
    let mut named: Vec<Value> = world
        .query::<Name>()
        .filter_map(|(entity, name)| {
            let id = world.get::<SceneId>(entity)?;
            let at = world
                .get::<Transform>(entity)
                .map(|t| [t.translation.x, t.translation.y]);
            Some(json!({ "id": id.0.to_string(), "name": name.as_str(), "position": at }))
        })
        .collect();
    named.truncate(MAX_SUMMARY);
    Value::Array(named)
}

/// A screenshot's size as asked, within one pixel and [`MAX_SHOT_SIDE`].
fn shot_size(width: u32, height: u32) -> (u32, u32) {
    (
        width.clamp(1, MAX_SHOT_SIDE),
        height.clamp(1, MAX_SHOT_SIDE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_draws_the_hud_s_buttons_with_their_faces() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let project = Project::new(root, None);
        let outcome = run_scene(
            &project,
            &RunRequest {
                scene: "scenes/platformer.ron".into(),
                frames: 2,
                delta: 1.0 / 60.0,
                inputs: Vec::new(),
                report: Vec::new(),
                screenshot: Some((640, 360)),
                profile: false,
            },
        )
        .expect("runs");
        let Some(png) = outcome.png else {
            // No adapter on this machine: nothing drawn to look at.
            return;
        };
        let shot = image::load_from_memory(&png).expect("a png").to_rgba8();
        // Inside the Save button, clear of its label: its face, not the
        // sprite's own white.
        let pixel = shot.get_pixel(20, 72).0;
        assert!(
            pixel[..3].iter().all(|&c| c < 160),
            "the button's face, got {pixel:?}"
        );
    }
}
