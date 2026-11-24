//! `cargo xtask pack-atlas`: a folder of PNGs into one sheet and its
//! `.atlas.ron`, by the packer the editor uses too
//! (`voltra_assets::packer`).

use std::path::{Path, PathBuf};

use voltra_assets::packer::{default_atlas_path, pack_folder, PackOptions};

/// Parses the arguments after `pack-atlas` and packs.
pub fn run(mut args: impl Iterator<Item = String>, workspace: &Path) -> Result<(), String> {
    let mut folder = None;
    let mut out = None;
    let mut root = None;
    let mut options = PackOptions::default();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value()?)),
            "--root" => root = Some(PathBuf::from(value()?)),
            "--padding" => options.padding = number(&arg, &value()?)?,
            "--extrude" => options.extrude = number(&arg, &value()?)?,
            "--max-size" => options.max_size = number(&arg, &value()?)?.max(1),
            "--trim" => options.trim = true,
            other if other.starts_with("--") => {
                return Err(format!("unknown option `{other}`\n\n{}", crate::USAGE))
            }
            other if folder.is_none() => folder = Some(PathBuf::from(other)),
            other => return Err(format!("one folder only, not `{other}` too")),
        }
    }
    let folder = folder.ok_or(format!("pack-atlas needs a folder\n\n{}", crate::USAGE))?;
    if !folder.is_dir() {
        return Err(format!("{} is not a folder", folder.display()));
    }
    let atlas = out.unwrap_or_else(|| default_atlas_path(&folder));
    let root = root.unwrap_or_else(|| workspace.join("assets"));

    let report = pack_folder(&folder, &atlas, Some(&root), &options).map_err(|e| e.to_string())?;
    println!(
        "packed {} regions into a {}×{} sheet:\n  {}\n  {}",
        report.regions,
        report.size.x,
        report.size.y,
        report.sheet.display(),
        report.atlas.display()
    );
    match &report.texture {
        Some(texture) => println!("texture hint: {}", texture.as_str()),
        None => println!(
            "no texture hint: the sheet is not under {} (pass --root)",
            root.display()
        ),
    }
    for gone in &report.gone {
        println!("kept an empty slot for {gone:?}, whose file is gone");
    }
    Ok(())
}

fn number(option: &str, text: &str) -> Result<u32, String> {
    text.parse()
        .map_err(|_| format!("`{option}` wants a whole number, not `{text}`"))
}
