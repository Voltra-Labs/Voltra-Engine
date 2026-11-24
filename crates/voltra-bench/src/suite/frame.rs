//! A whole frame, drawn headless: the screenshot path of `voltra-core`.

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, ScreenshotError, WindowConfig};

use super::{assets, Bench, Outcome};
use crate::harness::{iter, measure};

/// The shot's size: small, so the time is the CPU's recording of the frame
/// more than a software rasterizer filling pixels.
const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

pub(super) fn benches() -> Vec<Bench> {
    vec![Bench::new(
        "frame/headless/sandbox-640x360",
        // lavapipe rasterizes on every core, so a busy machine slows it most:
        // the noisiest benchmark measured (stage 107).
        65.0,
        |config| {
            let app = App::new(WindowConfig::default()).with_asset_root(assets());
            let scene = AssetPath::new("scenes/sandbox.ron").expect("a valid asset path");
            let mut session = match Headless::open(app, &scene) {
                Ok(session) => session,
                Err(e) => return Outcome::Skipped(format!("the sandbox would not load: {e}")),
            };
            // The first shot makes the device and uploads the textures:
            // a one-off, and where a machine with no adapter is found.
            match session.screenshot(WIDTH, HEIGHT) {
                Ok(_) => {}
                Err(ScreenshotError::NoDevice) => {
                    return Outcome::Skipped("no GPU adapter".to_owned())
                }
                Err(e) => return Outcome::Skipped(e.to_string()),
            }
            measure(
                config,
                1,
                iter(|| session.screenshot(WIDTH, HEIGHT).map(|shot| shot.width)),
            )
            .into()
        },
    )]
}
