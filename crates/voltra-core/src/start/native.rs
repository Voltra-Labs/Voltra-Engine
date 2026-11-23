//! Starting on a desktop: the files are already on disk.

/// Runs `game` now. An error it returns is logged: a game that cannot
/// start says why rather than leaving no window and no word.
pub fn when_assets_ready(game: impl FnOnce() -> Result<(), String> + 'static) {
    if let Err(error) = game() {
        log::error!("{error}");
    }
}
