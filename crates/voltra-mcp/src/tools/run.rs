//! Running a scene headless for a while, and reporting what came of it.

use serde::Deserialize;
use serde_json::{json, Value};
use voltra_core::{App, Headless, ScreenshotError, WindowConfig};
use voltra_scene::{Name, SceneId, Transform};

use super::ToolError;
use crate::Project;

/// The most frames one run may ask for: a minute and a half at 60 Hz.
pub const MAX_FRAMES: u32 = 6000;
/// The most log lines a run hands back; the latest are kept.
const MAX_LOG_LINES: usize = 200;
/// The most entities a run with no report list sums up.
const MAX_SUMMARY: usize = 100;

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
    pub scene: String,
    pub frames: u32,
    pub delta: f32,
    pub inputs: Vec<HeldAction>,
    pub report: Vec<String>,
    pub screenshot: Option<(u32, u32)>,
    /// Whether to time the run's frames, scope by scope.
    pub profile: bool,
}

/// What came of a run: the report, and the screenshot as PNG bytes.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub report: Value,
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
    let app = App::new(WindowConfig::default()).with_asset_root(project.assets());
    let mut session =
        Headless::open(app, &asset).map_err(|e| ToolError(format!("{}: {e}", asset.as_str())))?;

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
            match session.screenshot(width.clamp(1, 2048), height.clamp(1, 2048)) {
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
