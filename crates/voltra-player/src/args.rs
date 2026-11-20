//! What a build is told on the way in.
//!
//! A player has no menus, so everything it needs — which scene, where the
//! assets are, how big the window opens — arrives as arguments. Godot's export
//! templates take `--main-pack`, Unreal's shipped binary takes a map name and
//! `-ResX`/`-ResY`, and Unity bakes the same set into the build. The build
//! manifest (`build.ron`) is the shape those converge on; `launch` fills in
//! from it whatever this leaves unsaid.

use std::fmt;
use std::path::PathBuf;

/// What the player was asked to do.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    /// Open a window and run the scene.
    Run(Args),
    /// Print [`USAGE`] and stop. `--help` is not a failure.
    Help,
}

/// A parsed command line.
#[derive(Debug, Default, PartialEq)]
pub struct Args {
    /// The scene to run. `None` runs the build the manifest describes.
    pub scene: Option<PathBuf>,
    /// A build manifest to read instead of the one beside the player.
    pub manifest: Option<PathBuf>,
    /// Overrides the asset root every asset path resolves against.
    ///
    /// `None` leaves `voltra_assets::default_root`'s rules alone, which
    /// already cover both a `cargo run` and a binary sitting beside its own
    /// `assets` directory.
    pub asset_root: Option<PathBuf>,
    /// Window title. `None` means the scene file's stem.
    pub title: Option<String>,
    /// Window size in logical pixels. `None` means the platform default.
    pub size: Option<(u32, u32)>,
}

pub const USAGE: &str = "\
Usage: voltra-player [OPTIONS] [SCENE]

Arguments:
  [SCENE]               Path to the .ron scene to run (default: the scene
                        build.ron beside the player names)

Options:
      --manifest FILE   Build manifest to read instead of ./build.ron
      --asset-root DIR  Directory every asset path resolves against
      --title TEXT      Window title (default: the scene file's name)
      --size WxH        Window size in logical pixels, e.g. 1280x720
  -h, --help            Print this message";

/// Everything a command line can get wrong.
#[derive(Debug, PartialEq)]
pub enum ArgError {
    /// A second positional argument. Loading two scenes into one world is a
    /// real operation — additive loading — and not one this stage decided on,
    /// so it is refused rather than guessed at.
    ExtraScene(String),
    /// A flag that takes a value and did not get one.
    MissingValue(&'static str),
    /// `--size` with something that is not `WxH`.
    BadSize(String),
    /// A flag nobody knows. Refused rather than ignored: a typo that silently
    /// does nothing is worse than one that says so.
    UnknownFlag(String),
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExtraScene(extra) => {
                write!(f, "only one scene can be run, and `{extra}` is a second")
            }
            Self::MissingValue(flag) => write!(f, "`{flag}` needs a value"),
            Self::BadSize(value) => write!(f, "`--size` wants WxH in whole pixels, got `{value}`"),
            Self::UnknownFlag(flag) => write!(f, "unknown option `{flag}`"),
        }
    }
}

impl std::error::Error for ArgError {}

/// Parses the arguments *after* the executable's own name.
pub fn parse<I>(args: I) -> Result<Parsed, ArgError>
where
    I: IntoIterator<Item = String>,
{
    let mut parsed = Args::default();
    let mut scene: Option<PathBuf> = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--asset-root" => {
                let value = args.next().ok_or(ArgError::MissingValue("--asset-root"))?;
                parsed.asset_root = Some(PathBuf::from(value));
            }
            "--manifest" => {
                let value = args.next().ok_or(ArgError::MissingValue("--manifest"))?;
                parsed.manifest = Some(PathBuf::from(value));
            }
            "--title" => {
                let value = args.next().ok_or(ArgError::MissingValue("--title"))?;
                parsed.title = Some(value);
            }
            "--size" => {
                let value = args.next().ok_or(ArgError::MissingValue("--size"))?;
                parsed.size = Some(parse_size(&value)?);
            }
            // Anything starting with a dash is read as an option. A path may
            // legally start with one, but a mistyped flag is the failure that
            // would otherwise pass silently, and it is the more likely of the
            // two by a wide margin.
            other if other.starts_with('-') => {
                return Err(ArgError::UnknownFlag(other.to_string()))
            }
            other => match scene {
                None => scene = Some(PathBuf::from(other)),
                Some(_) => return Err(ArgError::ExtraScene(other.to_string())),
            },
        }
    }

    parsed.scene = scene;
    Ok(Parsed::Run(parsed))
}

/// `WxH`, in whole pixels, both sides non-zero.
///
/// A zero would reach `winit` as a window of no size, which every platform
/// answers differently; refusing it here means there is one answer.
fn parse_size(value: &str) -> Result<(u32, u32), ArgError> {
    let bad = || ArgError::BadSize(value.to_string());
    // Either case of `x`: `1280X720` is the same intent typed with the shift
    // key down.
    let (width, height) = value.split_once(['x', 'X']).ok_or_else(bad)?;
    let width: u32 = width.trim().parse().map_err(|_| bad())?;
    let height: u32 = height.trim().parse().map_err(|_| bad())?;
    if width == 0 || height == 0 {
        return Err(bad());
    }
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(args: &[&str]) -> Result<Parsed, ArgError> {
        parse(args.iter().map(|arg| arg.to_string()))
    }

    fn run(args: &[&str]) -> Args {
        match parse_str(args).expect("a valid command line") {
            Parsed::Run(args) => args,
            Parsed::Help => panic!("expected a run, got help"),
        }
    }

    #[test]
    fn the_scene_is_the_one_positional_argument() {
        assert_eq!(
            run(&["scenes/level.ron"]).scene,
            Some(PathBuf::from("scenes/level.ron"))
        );
    }

    #[test]
    fn no_scene_is_a_run_of_the_manifest_not_an_error() {
        // Which manifest, and whether there is one, is `launch`'s question.
        assert_eq!(run(&[]).scene, None);
    }

    #[test]
    fn a_manifest_can_be_named() {
        let args = run(&["--manifest", "builds/demo.ron"]);
        assert_eq!(args.manifest, Some(PathBuf::from("builds/demo.ron")));
        assert_eq!(
            parse_str(&["--manifest"]),
            Err(ArgError::MissingValue("--manifest"))
        );
    }

    #[test]
    fn a_second_scene_is_refused_rather_than_ignored() {
        assert_eq!(
            parse_str(&["a.ron", "b.ron"]),
            Err(ArgError::ExtraScene("b.ron".to_string()))
        );
    }

    #[test]
    fn the_options_are_read_in_any_order() {
        let args = run(&[
            "--title",
            "Sandbox",
            "level.ron",
            "--size",
            "800x600",
            "--asset-root",
            "build/assets",
        ]);

        assert_eq!(args.scene, Some(PathBuf::from("level.ron")));
        assert_eq!(args.title.as_deref(), Some("Sandbox"));
        assert_eq!(args.size, Some((800, 600)));
        assert_eq!(args.asset_root, Some(PathBuf::from("build/assets")));
    }

    #[test]
    fn help_wins_over_everything_after_it() {
        assert_eq!(parse_str(&["--help", "--nonsense"]), Ok(Parsed::Help));
        assert_eq!(parse_str(&["-h"]), Ok(Parsed::Help));
    }

    #[test]
    fn a_flag_that_lost_its_value_is_refused() {
        assert_eq!(
            parse_str(&["level.ron", "--size"]),
            Err(ArgError::MissingValue("--size"))
        );
        assert_eq!(
            parse_str(&["level.ron", "--asset-root"]),
            Err(ArgError::MissingValue("--asset-root"))
        );
    }

    #[test]
    fn an_unknown_flag_is_refused() {
        assert_eq!(
            parse_str(&["--fullscreen", "level.ron"]),
            Err(ArgError::UnknownFlag("--fullscreen".to_string()))
        );
    }

    #[test]
    fn a_size_is_two_whole_non_zero_numbers() {
        assert_eq!(parse_size("1280x720"), Ok((1280, 720)));
        assert_eq!(parse_size("1280X720"), Ok((1280, 720)));
        assert!(parse_size("1280").is_err());
        assert!(parse_size("1280x").is_err());
        assert!(parse_size("0x720").is_err());
        assert!(parse_size("1280x-720").is_err());
        assert!(parse_size("12.8x72").is_err());
    }
}
