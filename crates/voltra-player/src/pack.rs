//! Where a web build's game comes from: one pack, fetched beside the page.
//!
//! The pack is the native build folder without the executable — the shipped
//! manifest at its top and the asset tree under it — so the web player
//! resolves its launch exactly as a native one does from beside its
//! executable, with the pack mounted where the executable's folder would be.

/// The pack's file name, beside the page that runs it.
pub const PACK_FILE: &str = "game.pak";

/// Where the web player mounts the pack: the folder a native build's
/// executable sits in. Absolute, so no relative path a scene names can be
/// read against anything else.
pub const PACK_ROOT: &str = "/game";
