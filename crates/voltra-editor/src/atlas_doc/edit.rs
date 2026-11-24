//! The edits a window or the bridge makes to an open atlas, each one undo
//! entry.

use voltra_assets::Frame;
use voltra_render::glam::{UVec2, Vec2};

use super::file::stem;
use super::{slice, AtlasDoc, Region};

impl AtlasDoc {
    /// Replaces every region with the cells of a grid; see [`slice::grid`].
    /// A cell named as a region was keeps that region's pivot. Returns how
    /// many cells there are.
    pub fn slice_grid(
        &mut self,
        cell: UVec2,
        offset: UVec2,
        spacing: UVec2,
    ) -> Result<usize, String> {
        let stem = stem(&self.texture);
        let frames = slice::grid(self.sheet, cell, offset, spacing)?;
        let regions: Vec<Region> = frames
            .into_iter()
            .enumerate()
            .map(|(i, frame)| {
                let name = format!("{stem}_{i}");
                let pivot = self
                    .regions
                    .iter()
                    .find(|region| region.name == name)
                    .and_then(|region| region.pivot);
                Region {
                    name,
                    frame,
                    pivot,
                    trim: None,
                }
            })
            .collect();
        let count = regions.len();
        self.regions = regions;
        self.selected = None;
        self.settle(true);
        Ok(count)
    }

    /// Adds a region over `frame`, named `name` or the first free
    /// `region_N`. Returns its index.
    pub fn add(&mut self, frame: Frame, name: Option<&str>) -> Result<usize, String> {
        let frame = self.clamped(frame)?;
        let name = match name {
            Some(name) => {
                self.check_free(name, None)?;
                name.to_owned()
            }
            None => (self.regions.len()..)
                .map(|n| format!("region_{n}"))
                .find(|name| self.check_free(name, None).is_ok())
                .expect("an unbounded range holds a free name"),
        };
        self.regions.push(Region {
            name,
            frame,
            pivot: None,
            trim: None,
        });
        self.selected = Some(self.regions.len() - 1);
        self.settle(true);
        Ok(self.regions.len() - 1)
    }

    /// Renames region `index`. Refused for a name another region has: the
    /// packer and `Atlas::index_of` find a region by it.
    pub fn rename(&mut self, index: usize, name: &str) -> Result<(), String> {
        self.check_index(index)?;
        if !name.is_empty() {
            self.check_free(name, Some(index))?;
        }
        self.regions[index].name = name.to_owned();
        self.settle(true);
        Ok(())
    }

    /// Moves or resizes region `index` to `frame`, kept on the sheet.
    pub fn set_frame(&mut self, index: usize, frame: Frame) -> Result<(), String> {
        self.check_index(index)?;
        let frame = self.clamped(frame)?;
        let region = &mut self.regions[index];
        region.frame = frame;
        // A trim describes the picture the old rectangle was cut from.
        region.trim = None;
        self.settle(true);
        Ok(())
    }

    /// Sets region `index`'s pivot, clamped to `0..1`; `None` is the centre.
    pub fn set_pivot(&mut self, index: usize, pivot: Option<Vec2>) -> Result<(), String> {
        self.check_index(index)?;
        if pivot.is_some_and(|pivot| !pivot.is_finite()) {
            return Err("a pivot is two numbers from 0 to 1".into());
        }
        self.regions[index].pivot = pivot.map(|pivot| pivot.clamp(Vec2::ZERO, Vec2::ONE));
        self.settle(true);
        Ok(())
    }

    /// Removes region `index`. The ones after it move down an index, which
    /// sprites naming them by index will see: said here, so the window can.
    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        self.check_index(index)?;
        self.regions.remove(index);
        self.settle(true);
        Ok(())
    }

    fn check_index(&self, index: usize) -> Result<(), String> {
        if index < self.regions.len() {
            Ok(())
        } else {
            Err(format!(
                "no region {index}: the atlas has {}",
                self.regions.len()
            ))
        }
    }

    fn check_free(&self, name: &str, except: Option<usize>) -> Result<(), String> {
        match self.regions.iter().position(|r| r.name == name) {
            Some(other) if Some(other) != except => {
                Err(format!("region {other} is already called {name:?}"))
            }
            _ => Ok(()),
        }
    }

    /// `frame` cut to the sheet; refused if nothing of it is on the sheet.
    fn clamped(&self, frame: Frame) -> Result<Frame, String> {
        let x = frame.x.min(self.sheet.x);
        let y = frame.y.min(self.sheet.y);
        let right = frame.x.saturating_add(frame.width).min(self.sheet.x);
        let bottom = frame.y.saturating_add(frame.height).min(self.sheet.y);
        let clamped = Frame::new(x, y, right - x, bottom - y);
        if clamped.is_empty() {
            return Err(format!(
                "a region of {}×{} at ({}, {}) has no texels on the {}×{} sheet",
                frame.width, frame.height, frame.x, frame.y, self.sheet.x, self.sheet.y
            ));
        }
        Ok(clamped)
    }
}
