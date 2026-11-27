//! Interest management: which replicated entities each client is sent.
//!
//! An entity is *relevant* to a client when it is within a radius of one
//! of the client's interest points — by default where the entities it owns
//! ([`NetOwner`](crate::NetOwner)) are, plus any the game sets
//! ([`ReplicationServer::set_interest_points`]) — or when it is marked
//! [`Replicated::always_relevant`]. One that is not costs that client
//! nothing: no bytes, no place in its budget, no copy in its world.
//!
//! Unreal's `NetCullDistanceSquared` with its replication graph's grid
//! node: the question "what is near this point" is answered by a uniform
//! grid built once a snapshot ([`InterestGrid`]), so each client looks at
//! the cells around its points rather than at every entity — per-pair
//! distance is clients × entities, every tick, and the grid is entities
//! once plus each client's neighbourhood.
//!
//! [`ReplicationServer::set_interest_points`]: crate::ReplicationServer::set_interest_points
//! [`Replicated::always_relevant`]: voltra_scene::Replicated::always_relevant

mod grid;

pub use grid::InterestGrid;

/// The most cells a client's query may span along one axis: a cell size
/// set far below the radius is raised until the leave radius's diameter is
/// at most this many cells, so a query's range stays a range and not
/// billions of empty cells to step over.
pub const MAX_CELLS_PER_AXIS: f32 = 65_536.0;

/// How far a client is sent the world: [`ReplicationConfig::interest`].
/// `None` there sends every client every replicated entity.
///
/// [`ReplicationConfig::interest`]: crate::ReplicationConfig::interest
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interest {
    /// An entity this close to one of a client's interest points, in world
    /// units, becomes relevant to it.
    pub radius: f32,
    /// The side of the grid's square cells, in world units. About the
    /// radius is right: a query then reads the 3×3 cells around a point.
    /// Much smaller reads many empty cells, much larger many far entities.
    pub cell_size: f32,
    /// How much further than `radius`, as a share of it, a relevant entity
    /// may go before it stops being relevant: `0.1` enters at the radius
    /// and leaves at 1.1 × the radius, so an entity on the edge is not
    /// despawned and spawned again every tick.
    pub hysteresis: f32,
}

impl Interest {
    /// Relevance within `radius`, on cells of that size, with the default
    /// hysteresis of 10 %.
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            cell_size: radius,
            hysteresis: 0.1,
        }
    }

    /// The distance a relevant entity stops being relevant at, for an
    /// entity relevant within `radius`.
    pub fn leave_radius(&self, radius: f32) -> f32 {
        radius * (1.0 + self.hysteresis.max(0.0))
    }

    /// The same with every value usable: a radius that is not a positive
    /// number, or a leave radius that is not finite (a radius or a
    /// hysteresis near `f32::MAX`), is refused (`None`, with an error
    /// logged); a cell size that is not a positive number is the radius;
    /// one so small a query would span more than [`MAX_CELLS_PER_AXIS`]
    /// cells is raised to fit; a negative or non-finite hysteresis is none.
    pub(crate) fn checked(self) -> Option<Self> {
        if !(self.radius.is_finite() && self.radius > 0.0) {
            log::error!(
                "interest radius {} is not a positive distance; every client is sent everything",
                self.radius
            );
            return None;
        }
        let hysteresis = if self.hysteresis.is_finite() {
            self.hysteresis.max(0.0)
        } else {
            0.0
        };
        let leave = self.radius * (1.0 + hysteresis);
        // The span a query reads, both sides of a point.
        let reach = leave * 2.0;
        if !reach.is_finite() {
            log::error!(
                "interest radius {} with hysteresis {} leaves at no finite distance; \
                 every client is sent everything",
                self.radius,
                hysteresis
            );
            return None;
        }
        let mut cell_size = if self.cell_size.is_finite() && self.cell_size > 0.0 {
            self.cell_size
        } else {
            log::warn!(
                "interest cell size {} is not a positive distance; the radius, {}, is used",
                self.cell_size,
                self.radius
            );
            self.radius
        };
        if reach / cell_size > MAX_CELLS_PER_AXIS {
            let raised = reach / MAX_CELLS_PER_AXIS;
            log::warn!(
                "interest cell size {cell_size} would have a query read over \
                 {MAX_CELLS_PER_AXIS} cells a side; {raised} is used"
            );
            cell_size = raised;
        }
        Some(Self {
            radius: self.radius,
            cell_size,
            hysteresis,
        })
    }
}

impl Default for Interest {
    /// 30 world units: six screens of a camera 5 units tall.
    fn default() -> Self {
        Self::new(30.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_that_cannot_work_are_fixed_or_refused() {
        assert_eq!(Interest::new(f32::NAN).checked(), None);
        assert_eq!(Interest::new(-1.0).checked(), None);
        let fixed = Interest {
            radius: 10.0,
            cell_size: 0.0,
            hysteresis: -3.0,
        }
        .checked()
        .expect("a usable radius");
        assert_eq!(fixed.cell_size, 10.0);
        assert_eq!(fixed.hysteresis, 0.0);
        assert!((Interest::new(10.0).leave_radius(10.0) - 11.0).abs() < 1e-5);
    }

    #[test]
    fn a_leave_radius_past_f32_is_refused_and_a_tiny_cell_raised() {
        assert_eq!(Interest::new(f32::MAX).checked(), None);
        let hysteresis = Interest {
            hysteresis: f32::MAX,
            ..Interest::new(10.0)
        };
        assert_eq!(hysteresis.checked(), None);
        let huge = Interest {
            hysteresis: 1.0e30,
            ..Interest::new(10.0)
        }
        .checked()
        .expect("finite");
        assert!(huge.leave_radius(huge.radius) * 2.0 / huge.cell_size <= MAX_CELLS_PER_AXIS);
        let fine = Interest {
            cell_size: 1.0e-3,
            ..Interest::new(1.0e7)
        }
        .checked()
        .expect("finite");
        assert!(fine.leave_radius(fine.radius) * 2.0 / fine.cell_size <= MAX_CELLS_PER_AXIS);
        assert!(fine.cell_size > 300.0, "{}", fine.cell_size);
    }
}
