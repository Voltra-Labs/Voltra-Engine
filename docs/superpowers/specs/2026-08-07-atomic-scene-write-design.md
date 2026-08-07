# Atomic scene write — design

Date: 2026-08-07
Status: approved

## Problem

`voltra_scene::format::save` ends in `std::fs::write(path, text)`, which
truncates the destination and then writes into it. Between those two steps the
file on disk is shorter than both the old scene and the new one. A crash, a
process kill, a full disk or an I/O error inside that window leaves a truncated
scene file, and the work that was in the old file is gone — there is no copy of
it anywhere.

This is the only place in the workspace that writes user data to disk, so it is
the only place that can currently lose any.

## Guarantee

After this change, a call to `save` has exactly two outcomes:

- **`Ok(())`** — the file at `path` holds the complete new scene.
- **`Err(_)`** — the file at `path` is byte-for-byte what it was before the
  call, or still absent if it was absent.

There is no third state. A reader opening the file concurrently sees one whole
version or the other, never a partial one.

## Approach

Write the new contents to a temporary file beside the destination, flush it to
the physical disk, then rename it over the destination. `rename` replacing an
existing file is the only filesystem operation that swaps a file's contents
without a window in which neither version is complete.

Three details that the approach only works because of:

**The temporary must be a sibling of the destination, not in the system temp
directory.** `rename` is atomic only within a single volume; across volumes it
degrades to a copy, which reintroduces the window this change exists to close.

**`sync_all` before the rename, not after.** Without it the rename can reach
the disk before the bytes do, and a power loss leaves the file renamed and
empty — the original destroyed and the replacement never written. `sync_all`
costs a few milliseconds and a scene save happens on a human keypress, so the
cost is invisible.

**The temporary name is unique per write.** `<file_name>.<uuid-v7>.tmp`, reusing
the `uuid` dependency the crate already has. Two writers of the same
destination — two editor instances open on one project — then never share a
temporary, so neither can truncate the other's half-written file. Each renames
its own and the last one wins, which is what "last write wins" is supposed to
mean.

### Rejected

**In-place write with a `.bak` copy of the previous version.** Turns one
window into two — there is still a moment when the destination is truncated,
and now also a moment when the backup is. Scene files are committed to git, so
the previous version already has a better home than a sibling `.bak`.

**The `tempfile` crate.** `NamedTempFile::persist` does exactly this and is
well tested. Rejected because the whole thing is about sixty lines against
`std`, the crate pulls `fastrand` plus `rustix`/`windows-sys` transitively, and
it chooses the temporary's name itself. That last point stops being cosmetic in
stage 12, when a hot-reload watcher starts looking at `assets/`: the name of the
file that appears and vanishes during every save is something this engine wants
to control.

**A fixed temporary name, `scene.ron.tmp`, which is what Godot uses.** It
self-cleans — the next write reuses the same path, so at most one orphan ever
exists. Rejected because two concurrent writers then share one temporary and
corrupt each other, which is the same failure this work exists to close, moved
one level down. Godot lives with it and has the matching bug report
([godotengine/godot#956](https://github.com/godotengine/godot/issues/956)).

## Shape

A new module, `crates/voltra-scene/src/format/atomic.rs`. `save.rs` is already
214 lines, and describing it afterwards would need the word "and": it turns a
world into RON text *and* gets bytes onto disk without a truncation window.
Those are two concepts, so they are two files.

```rust
/// Replaces `path`'s contents with `bytes`, or leaves the file untouched.
pub fn replace(path: &Path, bytes: &[u8]) -> std::io::Result<()>
```

`&[u8]` rather than `&str`. The second caller is already planned — the asset
cache in stage 12 writes binary — and widening the parameter costs nothing
today.

Steps:

1. Derive the temporary as `path.with_file_name(format!("{name}.{id}.tmp"))`.
   `with_file_name` handles a destination with an empty parent, so no `unwrap`
   is needed to reach the directory.
2. Arm a guard owning the temporary path. Its `Drop` removes the file unless
   the guard is disarmed, so every `?` below — including the one on the rename
   — cleans up after itself.
3. `File::create` → `write_all` → `sync_all` → close the handle.
4. `fs::rename(temp, path)`.
5. Disarm the guard.

### Cases handled when the code is written

| Case | Behaviour |
| --- | --- |
| Destination does not exist | Created. `rename` does not require a destination |
| Path has no file name (`..`, a root) | `io::ErrorKind::InvalidInput`, before touching the disk |
| Parent directory does not exist | `File::create` fails, destination untouched, no orphan left |
| `bytes` is empty | An empty file. Not a special case |
| Failure at any step | Guard removes the temporary, the error propagates as `SceneError::Io` |

`save` deliberately does **not** create a missing parent directory. That is a
separate decision from atomicity and would turn a mistyped path into a new
directory tree; the current `NotFound` error is the right answer and the editor
already logs it.

## Tests

In `atomic.rs`, under `#[cfg(test)] mod tests`, using `std::env::temp_dir()`
as `save.rs`'s tests already do:

- a file that did not exist is created
- existing contents are replaced
- **a failed replace leaves the original intact and no `.tmp` behind** — the
  test the change exists for
- a successful write leaves no temporary in the directory
- two temporaries derived for the same destination differ
- a path with no file name is rejected

## Known gap

The directory entry itself is not fsynced, because Windows has no portable
equivalent of an `fsync` on a directory handle. After a power loss the rename
may not have reached the disk even though the bytes did. The consequence is
losing that one save, never the previous file — which is the guarantee above,
intact. Recorded rather than papered over.
