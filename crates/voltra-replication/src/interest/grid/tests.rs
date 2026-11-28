use super::*;

fn grid(entities: &[(NetId, Vec2)]) -> InterestGrid {
    let mut grid = InterestGrid::new(Interest::new(10.0));
    for &(id, at) in entities {
        grid.insert(id, at, None);
    }
    grid
}

fn query(grid: &InterestGrid, points: &[Vec2], previous: &[NetId]) -> Vec<NetId> {
    let points: Vec<InterestPoint> = points.iter().map(|&p| p.into()).collect();
    seen_from(grid, &points, previous)
}

fn seen_from(grid: &InterestGrid, points: &[InterestPoint], previous: &[NetId]) -> Vec<NetId> {
    let previous: HashSet<NetId> = previous.iter().copied().collect();
    let mut out = HashSet::new();
    grid.relevant(points, &previous, &mut out);
    let mut out: Vec<NetId> = out.into_iter().collect();
    out.sort_unstable();
    out
}

#[test]
fn what_is_within_the_radius_of_any_point_and_nothing_else() {
    let grid = grid(&[
        (1, Vec2::new(5.0, 0.0)),
        (2, Vec2::new(-9.9, 0.0)),
        (3, Vec2::new(25.0, 0.0)),
        (4, Vec2::new(-200.0, -200.0)),
        (5, Vec2::new(100.0, 4.0)),
    ]);
    assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1, 2]);
    assert_eq!(
        query(&grid, &[Vec2::ZERO, Vec2::new(100.0, 0.0)], &[]),
        [1, 2, 5]
    );
    assert!(query(&grid, &[], &[1, 2]).is_empty(), "no point, nothing");
}

#[test]
fn a_relevant_entity_stays_until_past_the_leave_radius() {
    let grid = grid(&[(1, Vec2::new(10.5, 0.0)), (2, Vec2::new(11.5, 0.0))]);
    assert!(
        query(&grid, &[Vec2::ZERO], &[]).is_empty(),
        "outside: not entered"
    );
    assert_eq!(query(&grid, &[Vec2::ZERO], &[1, 2]), [1], "1 kept, 2 left");
}

#[test]
fn its_own_radius_reaches_past_the_cells_around_a_point() {
    let mut grid = grid(&[]);
    grid.insert(1, Vec2::new(80.0, 0.0), Some(100.0));
    grid.insert(2, Vec2::new(3.0, 0.0), Some(2.0));
    grid.insert(3, Vec2::new(1.0, 0.0), Some(f32::NAN));
    grid.insert(4, Vec2::new(f32::INFINITY, 0.0), None);
    assert_eq!(grid.len(), 3);
    assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1, 3]);
}

#[test]
fn a_tiny_cell_reads_the_occupied_cells_and_answers_the_same() {
    let mut fine = InterestGrid::new(Interest {
        cell_size: 0.001,
        ..Interest::new(10.0)
    });
    for (id, at) in [(1, Vec2::new(5.0, 5.0)), (2, Vec2::new(-30.0, 2.0))] {
        fine.insert(id, at, None);
    }
    assert_eq!(query(&fine, &[Vec2::ZERO], &[]), [1]);
}

#[test]
fn negative_coordinates_fall_in_their_own_cells() {
    let grid = grid(&[(1, Vec2::new(-0.5, -0.5)), (2, Vec2::new(-19.0, -1.0))]);
    assert_eq!(query(&grid, &[Vec2::new(-10.0, 0.0)], &[]), [1, 2]);
}

#[test]
fn a_rebuild_forgets_where_entities_were() {
    let mut grid = grid(&[(1, Vec2::new(1.0, 0.0))]);
    for step in 0..200 {
        grid.clear();
        grid.insert(1, Vec2::new(step as f32 * 15.0, 0.0), None);
    }
    assert_eq!(grid.len(), 1);
    assert!(query(&grid, &[Vec2::ZERO], &[]).is_empty());
    assert_eq!(query(&grid, &[Vec2::new(2985.0, 0.0)], &[]), [1]);
    assert!(grid.layers[0].cells.len() < 200, "left cells let go of");
}

#[test]
fn extreme_values_unchecked_answer_without_overflow_or_a_long_walk() {
    for interest in [
        Interest::new(f32::MAX),
        Interest {
            hysteresis: f32::MAX,
            ..Interest::new(10.0)
        },
        Interest {
            cell_size: 1.0e-3,
            ..Interest::new(1.0e7)
        },
    ] {
        let mut grid = InterestGrid::new(interest);
        grid.insert(1, Vec2::new(5.0, 0.0), None);
        grid.insert(2, Vec2::new(-1.0e6, 3.0e6), None);
        let started = std::time::Instant::now();
        let found = query(&grid, &[Vec2::ZERO], &[1, 2]);
        assert!(found.contains(&1), "{interest:?}: {found:?}");
        assert!(started.elapsed().as_secs() < 1, "{interest:?}");
    }
    for interest in [
        Interest::new(f32::MAX),
        Interest {
            cell_size: 1.0e-3,
            ..Interest::new(1.0e7)
        },
    ] {
        let Some(checked) = interest.checked() else {
            continue;
        };
        let mut grid = InterestGrid::new(checked);
        grid.insert(1, Vec2::new(9.0e6, 0.0), None);
        assert_eq!(query(&grid, &[Vec2::ZERO], &[]), [1]);
    }
}

#[test]
fn a_3d_entity_is_near_by_its_ground_plane_and_then_by_its_height() {
    use glam::Vec3;
    let mut grid = grid(&[]);
    // Radius 10, leave radius 11.
    grid.insert(1, Vec3::new(8.0, 0.0, 0.0), None);
    grid.insert(2, Vec3::new(0.0, 0.0, 12.0), None);
    grid.insert(3, Vec3::new(6.0, 9.0, 0.0), None);
    grid.insert(4, Vec3::new(0.0, 30.0, 0.0), None);
    grid.insert(5, Vec3::new(0.0, 0.0, 10.5), None);
    let origin = [InterestPoint::D3(Vec3::ZERO)];
    // 1 at 8 m across; 2 at 12 m along Z — (x, z), not (x, y); 3 at
    // 6 m across and 9 m up is 10.8 m away; 4 straight overhead at 30 m.
    assert_eq!(seen_from(&grid, &origin, &[]), [1]);
    assert_eq!(seen_from(&grid, &origin, &[3, 5]), [1, 3, 5], "inside 11 m");
    // A point up at 30 m sees the one overhead and nothing on the ground.
    assert_eq!(
        seen_from(&grid, &[Vec3::new(0.0, 30.0, 0.0).into()], &[]),
        [4]
    );
}

#[test]
fn a_point_sees_only_its_own_dimension() {
    use glam::Vec3;
    let mut grid = grid(&[(1, Vec2::new(1.0, 0.0))]);
    grid.insert(2, Vec3::new(1.0, 0.0, 0.0), None);
    grid.insert(3, Vec3::new(1.0, 0.0, 0.0), Some(100.0));
    grid.insert(4, Vec2::new(50.0, 0.0), Some(100.0));
    assert_eq!(query(&grid, &[Vec2::ZERO], &[2, 3]), [1, 4], "2D: no 3D");
    assert_eq!(
        seen_from(&grid, &[InterestPoint::D3(Vec3::ZERO)], &[1, 4]),
        [2, 3],
        "3D: no 2D, wide ones included"
    );
    assert_eq!(grid.len(), 4);
}
