use super::*;
use crate::batch::TriangleMesh;
use crate::curve::CurveKey;

/// Whether `p` is inside any triangle of `mesh`.
fn covers(mesh: &TriangleMesh, p: Vec2) -> bool {
    mesh.indices.chunks(3).any(|t| {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize].position);
        let d1 = (b - a).perp_dot(p - a);
        let d2 = (c - b).perp_dot(p - b);
        let d3 = (a - c).perp_dot(p - c);
        let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(negative && positive)
    })
}

/// A right angle: right one unit, then up one, a unit wide.
fn corner(joint: LineJoint) -> Line2D {
    Line2D {
        points: vec![Vec2::ZERO, Vec2::X, Vec2::ONE],
        width: 1.0,
        joint,
        ..Line2D::default()
    }
}

#[test]
fn a_straight_line_is_its_width_and_its_length() {
    let line = Line2D {
        points: vec![Vec2::ZERO, Vec2::new(2.0, 0.0)],
        width: 0.5,
        ..Line2D::default()
    };
    let mesh = line.mesh();
    assert!(covers(&mesh, Vec2::new(1.0, 0.2)));
    assert!(!covers(&mesh, Vec2::new(1.0, 0.3)));
    assert!(
        !covers(&mesh, Vec2::new(2.1, 0.0)),
        "a butt end stops at its point"
    );
}

#[test]
fn joints_fill_the_outer_corner_their_own_way() {
    // The outer corner of the turn is at (1.5, -0.5).
    let tip = Vec2::new(1.45, -0.45);
    let edge = Vec2::new(1.3, -0.3);
    let miter = corner(LineJoint::Miter).mesh();
    let bevel = corner(LineJoint::Bevel).mesh();
    let round = corner(LineJoint::Round).mesh();
    assert!(covers(&miter, tip), "a miter reaches the corner");
    assert!(
        !covers(&bevel, tip) && !covers(&bevel, edge),
        "a bevel cuts it off"
    );
    assert!(
        !covers(&round, tip) && covers(&round, edge),
        "a round one in between"
    );
}

#[test]
fn a_miter_past_its_limit_is_beveled() {
    // A hairpin: the miter would reach far past two half-widths.
    let line = Line2D {
        points: vec![Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(0.0, 0.4)],
        width: 0.2,
        ..Line2D::default()
    };
    let mesh = line.mesh();
    assert!(!covers(&mesh, Vec2::new(4.5, -0.05)));
}

#[test]
fn caps_extend_an_open_line_their_own_way() {
    let line = |cap| Line2D {
        points: vec![Vec2::ZERO, Vec2::new(2.0, 0.0)],
        width: 1.0,
        begin_cap: cap,
        end_cap: cap,
        ..Line2D::default()
    };
    let past = Vec2::new(2.3, 0.0);
    let corner = Vec2::new(2.45, 0.45);
    assert!(!covers(&line(LineCap::Butt).mesh(), past));
    let square = line(LineCap::Square).mesh();
    assert!(covers(&square, past) && covers(&square, corner));
    assert!(covers(&square, Vec2::new(-0.45, 0.45)), "at the start too");
    let round = line(LineCap::Round).mesh();
    assert!(covers(&round, past) && !covers(&round, corner));
    assert!(covers(&round, Vec2::new(-0.4, 0.0)));
}

#[test]
fn a_width_curve_tapers_the_line() {
    let line = Line2D {
        points: vec![Vec2::ZERO, Vec2::new(4.0, 0.0)],
        width: 1.0,
        width_curve: Some(Curve::new(vec![
            CurveKey::new(0.0, 1.0),
            CurveKey::new(1.0, 0.0),
        ])),
        ..Line2D::default()
    };
    let mesh = line.mesh();
    assert!(covers(&mesh, Vec2::new(0.5, 0.4)));
    assert!(!covers(&mesh, Vec2::new(3.5, 0.2)), "narrow near the end");
}

#[test]
fn a_tiled_texture_restarts_every_width() {
    let line = Line2D {
        points: vec![Vec2::ZERO, Vec2::new(2.5, 0.0)],
        width: 1.0,
        texture_mode: LineTexture::Tile,
        ..Line2D::default()
    };
    let mesh = line.mesh();
    let us: Vec<f32> = mesh.vertices.iter().map(|v| v.uv[0]).collect();
    assert!(us.iter().all(|u| (0.0..=1.0).contains(u)), "{us:?}");
    assert!(us.contains(&1.0), "a tile ends on u = 1");
    let stretched = Line2D {
        texture_mode: LineTexture::Stretch,
        ..line
    };
    let max = stretched
        .mesh()
        .vertices
        .iter()
        .map(|v| v.uv[0])
        .fold(0.0, f32::max);
    assert_eq!(max, 1.0);
}

#[test]
fn a_closed_line_joins_its_last_point_to_its_first() {
    let square = Line2D {
        points: vec![Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        width: 0.2,
        closed: true,
        ..Line2D::default()
    };
    let mesh = square.mesh();
    assert!(covers(&mesh, Vec2::new(0.0, 0.5)), "the closing edge");
    assert!(covers(&mesh, Vec2::new(-0.09, -0.09)), "its corner mitered");
    assert!(!covers(&mesh, Vec2::splat(0.5)), "hollow");
}

#[test]
fn nothing_to_draw_is_no_triangles() {
    for points in [
        vec![],
        vec![Vec2::ONE],
        vec![Vec2::ONE, Vec2::ONE],
        vec![Vec2::NAN, Vec2::X],
    ] {
        let line = Line2D {
            points,
            ..Line2D::default()
        };
        assert!(line.mesh().is_empty());
    }
    let zero = Line2D {
        width: f32::NAN,
        ..Line2D::default()
    };
    assert!(zero.mesh().vertices.iter().all(|v| v.position.is_finite()));
}

#[test]
fn a_line_round_trips_and_an_old_one_reads() {
    let line = Line2D {
        joint: LineJoint::Round,
        end_cap: LineCap::Square,
        width_curve: Some(Curve::linear(1.0, 0.5)),
        ..Line2D::default()
    };
    let text = ron::to_string(&line).expect("writes");
    assert_eq!(ron::from_str::<Line2D>(&text).expect("reads"), line);
    let minimal: Line2D =
        ron::from_str("(points:[(0.0,0.0),(1.0,0.0)],width:0.5,color:(1.0,1.0,1.0,1.0))")
            .expect("the defaults fill in");
    assert_eq!(minimal.miter_limit, 2.0);
}
