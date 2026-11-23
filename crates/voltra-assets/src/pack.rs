//! The asset pack: a game's files in one blob, for a target with no disk.
//!
//! A browser cannot open a file by path, so a web build fetches its whole
//! game — the manifest and the asset tree under it — as one file and
//! [`mount`](crate::fs::mount)s what [`decode`] returns. One fetch rather
//! than one per asset is Godot's `.pck` and Unity's WebGL `.data`: a page
//! knows how much there is to load before it starts, and a scene never waits
//! on a request mid-frame.
//!
//! Nothing clever: no compression (a web server gzips the response), no
//! index (the whole pack is read once). Little-endian throughout:
//!
//! ```text
//! "VPAK"  u32 version  u32 count
//! count × ( u32 path length  path (UTF-8, `/`-separated)  u64 length  bytes )
//! ```
//!
//! A pack is downloaded, so [`decode`] trusts none of it: every length is
//! checked against what is left before anything is allocated, and every path
//! must stay inside the pack.

use std::collections::HashSet;
use std::fmt;

/// The first four bytes of every pack.
pub const MAGIC: [u8; 4] = *b"VPAK";

/// The format version this build reads and writes.
pub const VERSION: u32 = 1;

/// The longest path a pack may hold, in bytes.
pub const MAX_PATH_LEN: usize = 1024;

/// Why bytes are not a pack this build can read.
#[derive(Debug, PartialEq, Eq)]
pub enum PackError {
    /// No `VPAK` at the start: not a pack at all — an HTML error page served
    /// in its place, most often.
    NotAPack,
    /// A pack from another version of the format.
    Version(u32),
    /// The bytes stop in the middle of something.
    Truncated,
    /// A path that is not UTF-8, is empty or too long, or would leave the
    /// pack: absolute, with `..`, `.`, an empty part or a backslash.
    BadPath(String),
    /// The same path twice.
    Duplicate(String),
    /// Bytes after the last file.
    TrailingBytes(usize),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAPack => write!(f, "not an asset pack (no VPAK header)"),
            Self::Version(v) => {
                write!(f, "asset pack version {v}; this build reads {VERSION}")
            }
            Self::Truncated => write!(f, "the asset pack ends early"),
            Self::BadPath(path) => write!(f, "`{path}` is not a path inside the pack"),
            Self::Duplicate(path) => write!(f, "`{path}` is in the pack twice"),
            Self::TrailingBytes(n) => write!(f, "{n} bytes after the last file in the pack"),
        }
    }
}

impl std::error::Error for PackError {}

/// Writes `files` — relative, `/`-separated paths and their bytes — as a
/// pack. Refuses what [`decode`] would: a pack that cannot be read back is
/// never written.
pub fn encode<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<Vec<u8>, PackError> {
    let files: Vec<_> = files.into_iter().collect();
    let mut seen = HashSet::with_capacity(files.len());
    let size: usize = files
        .iter()
        .map(|(path, bytes)| 12 + path.len() + bytes.len())
        .sum();
    let mut out = Vec::with_capacity(12 + size);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    let count = u32::try_from(files.len()).map_err(|_| PackError::Truncated)?;
    out.extend_from_slice(&count.to_le_bytes());
    for (path, bytes) in files {
        check_path(path)?;
        if !seen.insert(path) {
            return Err(PackError::Duplicate(path.to_owned()));
        }
        // `check_path` bounds it well under `u32::MAX`.
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path.as_bytes());
        out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

/// Reads a pack back into its files, in the order they were written.
pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, PackError> {
    let mut reader = Reader { rest: bytes };
    if reader.take(4).ok() != Some(&MAGIC[..]) {
        return Err(PackError::NotAPack);
    }
    let version = reader.u32()?;
    if version != VERSION {
        return Err(PackError::Version(version));
    }
    let count = reader.u32()? as usize;
    // Each entry is at least its two lengths: a count the bytes cannot hold
    // is refused before it sizes an allocation.
    if count > reader.rest.len() / 12 {
        return Err(PackError::Truncated);
    }
    let mut files = Vec::with_capacity(count);
    let mut seen = HashSet::with_capacity(count);
    for _ in 0..count {
        let path_len = reader.u32()? as usize;
        if path_len > MAX_PATH_LEN {
            return Err(PackError::BadPath(format!("<{path_len} bytes>")));
        }
        let path = std::str::from_utf8(reader.take(path_len)?)
            .map_err(|_| PackError::BadPath("<not UTF-8>".to_owned()))?
            .to_owned();
        check_path(&path)?;
        let len = usize::try_from(reader.u64()?).map_err(|_| PackError::Truncated)?;
        let data = reader.take(len)?.to_vec();
        if !seen.insert(path.clone()) {
            return Err(PackError::Duplicate(path));
        }
        files.push((path, data));
    }
    if !reader.rest.is_empty() {
        return Err(PackError::TrailingBytes(reader.rest.len()));
    }
    Ok(files)
}

/// A path that names a file inside the pack and nothing else.
fn check_path(path: &str) -> Result<(), PackError> {
    let bad = || PackError::BadPath(path.to_owned());
    if path.is_empty() || path.len() > MAX_PATH_LEN || path.contains('\\') {
        return Err(bad());
    }
    let inside = path
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'));
    if inside {
        Ok(())
    } else {
        Err(bad())
    }
}

struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], PackError> {
        if n > self.rest.len() {
            return Err(PackError::Truncated);
        }
        let (head, tail) = self.rest.split_at(n);
        self.rest = tail;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, PackError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("take(4) is four bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, PackError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("take(8) is eight bytes"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        encode([
            ("build.ron", &b"(version: 1)"[..]),
            ("assets/sprites/a.png", &[0u8, 1, 2, 255][..]),
            ("assets/empty", &[][..]),
        ])
        .expect("encodes")
    }

    #[test]
    fn a_pack_round_trips_in_order() {
        let files = decode(&sample()).expect("decodes");
        let paths: Vec<_> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, ["build.ron", "assets/sprites/a.png", "assets/empty"]);
        assert_eq!(files[1].1, [0, 1, 2, 255]);
        assert!(files[2].1.is_empty());
    }

    #[test]
    fn an_empty_pack_is_a_pack() {
        let bytes = encode(std::iter::empty()).expect("encodes");
        assert_eq!(decode(&bytes), Ok(Vec::new()));
    }

    #[test]
    fn what_is_not_a_pack_says_so() {
        assert_eq!(decode(b"<!doctype html>"), Err(PackError::NotAPack));
        assert_eq!(decode(b""), Err(PackError::NotAPack));
        let mut other = sample();
        other[4] = 9;
        assert_eq!(decode(&other), Err(PackError::Version(9)));
    }

    #[test]
    fn every_cut_short_pack_is_refused_without_panicking() {
        let bytes = sample();
        for end in 4..bytes.len() {
            assert!(decode(&bytes[..end]).is_err(), "cut at {end}");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(decode(&longer), Err(PackError::TrailingBytes(1)));
    }

    #[test]
    fn a_count_the_bytes_cannot_hold_is_refused_before_allocating() {
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(&bytes), Err(PackError::Truncated));
    }

    #[test]
    fn a_length_past_the_end_is_refused() {
        let mut bytes = encode([("a", &b"xy"[..])]).expect("encodes");
        // The data length sits right after the 1-byte path.
        let at = 12 + 4 + 1;
        bytes[at..at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(decode(&bytes), Err(PackError::Truncated));
    }

    #[test]
    fn paths_that_leave_the_pack_are_refused_both_ways() {
        for path in [
            "",
            "/etc/passwd",
            "../up",
            "a/../../b",
            "a/./b",
            "a//b",
            "a/",
            "a\\b",
            "C:/x",
        ] {
            assert!(
                matches!(encode([(path, &b""[..])]), Err(PackError::BadPath(_))),
                "{path:?} encoded"
            );
            // And a pack written by something else is held to the same rule.
            let mut bytes = MAGIC.to_vec();
            bytes.extend_from_slice(&VERSION.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&(path.len() as u32).to_le_bytes());
            bytes.extend_from_slice(path.as_bytes());
            bytes.extend_from_slice(&0u64.to_le_bytes());
            assert!(
                matches!(decode(&bytes), Err(PackError::BadPath(_))),
                "{path:?} decoded"
            );
        }
    }

    #[test]
    fn a_path_twice_is_refused() {
        assert_eq!(
            encode([("a", &b""[..]), ("a", &b""[..])]),
            Err(PackError::Duplicate("a".to_owned()))
        );
    }
}
