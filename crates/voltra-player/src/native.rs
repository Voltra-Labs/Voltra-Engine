//! The player on a desktop: its command line, the manifest beside it.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::args::{self, Parsed};
use crate::{launch, start};

pub fn main() -> ExitCode {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from));
    voltra_core::logging::init(cfg!(feature = "windowed"));

    // Skipping the executable's own path, which `parse` does not want to know
    // about — it is the one argument no user typed.
    let args = match args::parse(std::env::args().skip(1)) {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => {
            println!("{}", args::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            log::error!("{error}");
            println!("{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };

    let launch = match launch::resolve(&args, exe_dir.as_deref()) {
        Ok(launch) => launch,
        Err(error) => {
            log::error!("{error}");
            println!("{}", args::USAGE);
            return ExitCode::FAILURE;
        }
    };

    let Some(app) = start::app(launch) else {
        return ExitCode::FAILURE;
    };
    // Runs until the window closes. Anything after this line is unreachable
    // until `App::run` learns to return, so the exit code is the one above.
    app.run();
    ExitCode::SUCCESS
}
