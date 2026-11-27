//! Where a follower may be: the dead zone it aims through and the bounds
//! its view stays inside.

use voltra_render::glam::Vec2;

use super::FollowBounds;

/// Half the world box around a view of half size `half` turned by
/// `rotation`.
pub(super) fn seen_box(half: Vec2, rotation: f32) -> Vec2 {
    if rotation == 0.0 || !rotation.is_finite() {
        return half;
    }
    let (sin, cos) = rotation.sin_cos();
    Vec2::new(
        cos.abs() * half.x + sin.abs() * half.y,
        sin.abs() * half.x + cos.abs() * half.y,
    )
}

/// Where the centre should be for `aim` to sit inside a box of `half`
/// around it, moving as little as possible from `here`.
pub(super) fn dead_zone(here: Vec2, aim: Vec2, half: Vec2) -> Vec2 {
    let offset = aim - here;
    here + offset - offset.clamp(-half, half)
}

/// `centre` moved so a view of `half` around it stays inside `bounds`; a
/// view wider than the bounds is centred on them.
pub(super) fn clamp_to(centre: Vec2, bounds: Option<FollowBounds>, half: Vec2) -> Vec2 {
    let Some(FollowBounds { min, max }) = bounds else {
        return centre;
    };
    let (lo, hi) = (min.min(max), min.max(max));
    let mut out = centre;
    for axis in 0..2 {
        let (from, to) = (lo[axis] + half[axis], hi[axis] - half[axis]);
        out[axis] = if from > to {
            (lo[axis] + hi[axis]) * 0.5
        } else {
            centre[axis].clamp(from, to)
        };
    }
    out
}
