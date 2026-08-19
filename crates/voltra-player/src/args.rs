//! What a build is told on the way in.
//!
//! A player has no menus, so everything it needs — which scene, where the
//! assets are, how big the window opens — arrives as arguments. Godot's export
//! templates take `--main-pack`, Unreal's shipped binary takes a map name and
//! `-ResX`/`-ResY`, and Unity bakes the same set into the build. A project
//! settings file is the shape those converge on and is a stage of its own; the
//! command line is what that stage will read *into*, so the parsing lives
//! behind one type rather than inside `main`.

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
    /// The scene to run. The only required argument.
    pub scene: PathBuf,
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
Usage: voltra-player [OPTIONS] <SCENE>

Arguments:
  <SCENE>               Path to the .ron scene to run

Options:
      --asset-root DIR  Directory every asset path resolves against
      --title TEXT      Window title (default: the scene file's name)
      --size WxH        Window size in logical pixels, e.g. 1280x720
  -h, --help            Print this message";

/// Everything a command line can get wrong.
#[derive(Debug, PartialEq)]
pub enum ArgError {
    /// No scene, and a player with no scene has nothing to be.
    NoScene,
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
            Self::NoScene => write!(f, "no scene given"),
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

    parsed.scene = scene.ok_or(ArgError::NoScene)?;
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

/// The window title for `args`: what was asked for, or the scene's file name.
///
/// Godot titles a running project after the project rather than after the file
/// it booted. There is no project here yet, so the scene is the closest thing
/// to a name the player has been handed.
pub fn title(args: &Args) -> String {
    if let Some(title) = &args.title {
        return title.clone();
    }
    args.scene
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "Voltra".to_string())
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
            PathBuf::from("scenes/level.ron")
        );
    }

    #[test]
    fn a_player_without_a_scene_is_refused() {
        assert_eq!(parse_str(&[]), Err(ArgError::NoScene));
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

        assert_eq!(args.scene, PathBuf::from("level.ron"));
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

    #[test]
    fn the_title_falls_back_to_the_scene_name() {
        assert_eq!(title(&run(&["assets/scenes/sandbox.ron"])), "sandbox");
        assert_eq!(title(&run(&["--title", "My Game", "a.ron"])), "My Game");
    }
}
