//! The player in a browser: no command line, the manifest in the pack.
//!
//! `voltra_core::when_assets_ready` fetches and mounts the pack where a
//! native build's executable would sit; the launch is then resolved from
//! the manifest at its top exactly as a native build double-clicked
//! resolves it from the one beside it.

use std::path::Path;

use voltra_player::PACK_ROOT;

use crate::args::Args;
use crate::{launch, start};

pub fn main() {
    voltra_core::logging::init(false);
    voltra_core::when_assets_ready(|| {
        let launch = launch::resolve(&Args::default(), Some(Path::new(PACK_ROOT)))
            .map_err(|e| e.to_string())?;
        let app =
            start::app(launch).ok_or("the start scene could not be loaded; see the console")?;
        // Returns at once: the browser drives the loop from here.
        app.run();
        Ok(())
    });
}
