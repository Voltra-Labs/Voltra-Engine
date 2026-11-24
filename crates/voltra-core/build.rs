//! Records which compiler built this crate, for `game::stamp`: a game
//! library and the editor that loads it must have been built by the same
//! one, and the refusal should say which each was.

use std::process::Command;

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let version = Command::new(&rustc)
        .arg("-V")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|line| line.trim().to_owned())
        .unwrap_or_else(|| "an unknown rustc".to_owned());
    println!("cargo:rustc-env=VOLTRA_RUSTC_VERSION={version}");
    println!("cargo:rerun-if-env-changed=RUSTC");
}
