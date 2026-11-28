//! A text file's header: the comment block a writer keeps when it writes
//! the file again.

/// The comment lines (and the blank lines among them) `text` starts with:
/// what a tool rewriting an asset file — `cargo xtask migrate`, the
/// editor's material inspector — puts back in front of what it writes. A
/// file of blank lines and no comment has none.
pub fn leading_comments(text: &str) -> &str {
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            end += line.len();
        } else {
            break;
        }
    }
    // Leading blank lines alone are not a header.
    if text[..end].trim().is_empty() {
        ""
    } else {
        &text[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_is_the_leading_comments_only() {
        assert_eq!(
            leading_comments("// a\n// b\n\n(\n// c\n)"),
            "// a\n// b\n\n"
        );
        assert_eq!(leading_comments("(version: 2)"), "");
        assert_eq!(leading_comments("\n\n(version: 2)"), "");
    }
}
