//! The one RON layout every file the engine writes starts from.

/// `ron`'s pretty layout with `\n` line ends on every platform.
///
/// `PrettyConfig::default()` ends lines with `\r\n` on Windows, so a file
/// saved there differs byte for byte from the same file saved elsewhere, and a
/// trailing `\n` the writer appends makes it mixed. Every writer starts here
/// and adds only what its format needs (depth limit, struct names).
pub fn pretty_ron() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new().new_line("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_end_in_a_bare_newline_on_every_platform() {
        let text = ron::ser::to_string_pretty(&vec![(1, 2), (3, 4)], pretty_ron()).expect("writes");
        assert!(text.contains('\n'));
        assert!(!text.contains('\r'), "{text:?}");
    }
}
