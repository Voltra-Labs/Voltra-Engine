//! The log on a desktop: `env_logger`, to the terminal or a file.

use log::{LevelFilter, Log};

/// `env_logger` at `info` (or `RUST_LOG`), to [`LOG_FILE`](super::LOG_FILE)
/// with `to_file` when the file can be created, else to the terminal.
pub(super) fn platform_logger(to_file: bool) -> (Box<dyn Log>, LevelFilter) {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if to_file {
        if let Some(file) = super::log_path().and_then(|path| std::fs::File::create(path).ok()) {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
    }
    let logger = builder.build();
    let level = logger.filter();
    (Box::new(logger), level)
}
