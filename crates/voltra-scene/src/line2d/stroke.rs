//! A polyline widened into triangles: one quad strip per segment, a joint
//! on the outer side of every corner, caps on the ends.
//!
//! Each segment is its own strip, cut into pieces wherever the width curve
//! is sampled or a tile ends — so a width changes smoothly along one long
//! segment and a tiled texture restarts at each tile with the sprites'
//! clamped sampler. The inner sides of a corner overlap (a translucent line
//! is a little denser there, as Godot's is in its simple mode); the outer
//! side gets a miter, a bevel or a fan. `t` along the line is its length
//! so far over its whole length; `v` across is `0` on the left, `1` on the
//! right.

use std::f32::consts::PI;

use voltra_render::glam::Vec2;

use super::{LineCap, LineJoint, LineTexture};
use crate::batch::TriangleMesh;

/// How many pieces a whole line's width curve is sampled in.
const CURVE_PIECES: f32 = 32.0;

/// The most a round joint or cap turns per triangle.
const ROUND_STEP: f32 = PI / 8.0;

/// What to widen and how.
pub struct Stroke<'a> {
    /// The points, in the owner's space.
    pub points: &'a [Vec2],
    /// Whether the last point joins the first.
    pub closed: bool,
    /// The full width at `t` along the line.
    pub width: &'a dyn Fn(f32) -> f32,
    /// The colour at `t` along the line.
    pub color: &'a dyn Fn(f32) -> [f32; 4],
    /// How corners are joined.
    pub joint: LineJoint,
    /// How far a miter may reach, in half-widths.
    pub miter_limit: f32,
    /// What the first and the last point end in, for an open line.
    pub caps: (LineCap, LineCap),
    /// How the texture lies along it.
    pub texture: LineTexture,
    /// The length of one tile, for [`LineTexture::Tile`].
    pub tile: f32,
}

/// The triangles of `stroke`. Nothing for fewer than two distinct points,
/// a width that is nowhere positive, or anything not finite.
pub fn stroke(stroke: &Stroke<'_>) -> TriangleMesh {
    let mut points: Vec<Vec2> = stroke
        .points
        .iter()
        .copied()
        .filter(|p| p.is_finite())
        .collect();
    points.dedup_by(|b, a| a.distance_squared(*b) <= 1e-12);
    let closed = stroke.closed && points.len() >= 3;
    if closed && points[0].distance_squared(points[points.len() - 1]) <= 1e-12 {
        points.pop();
    }
    let mut mesh = TriangleMesh::default();
    if points.len() < 2 {
        return mesh;
    }
    let n = points.len();
    let segments = if closed { n } else { n - 1 };
    let ends = |i: usize| (points[i], points[(i + 1) % n]);
    let mut along = Vec::with_capacity(segments + 1);
    along.push(0.0);
    for i in 0..segments {
        let (a, b) = ends(i);
        along.push(along[i] + a.distance(b));
    }
    let total = along[segments];
    if !(total > 0.0 && total.is_finite()) {
        return mesh;
    }
    let ctx = Context {
        stroke,
        total,
        tile: if stroke.tile > 0.0 && stroke.tile.is_finite() {
            stroke.tile
        } else {
            total
        },
    };
    for i in 0..segments {
        let (a, b) = ends(i);
        ctx.segment(&mut mesh, a, b, along[i], along[i + 1]);
    }
    let corners: Vec<usize> = if closed {
        (0..n).collect()
    } else {
        (1..n - 1).collect()
    };
    for k in corners {
        let before = points[(k + n - 1) % n];
        let at = points[k];
        let after = points[(k + 1) % n];
        let s = if k == 0 { total } else { along[k] };
        ctx.joint(
            &mut mesh,
            at,
            (at - before).normalize(),
            (after - at).normalize(),
            s,
        );
    }
    if !closed {
        let first = (points[1] - points[0]).normalize();
        let last = (points[n - 1] - points[n - 2]).normalize();
        ctx.cap(&mut mesh, points[0], -first, 0.0, stroke.caps.0);
        ctx.cap(&mut mesh, points[n - 1], last, total, stroke.caps.1);
    }
    // A width that was nowhere positive, or turned into NaN, draws nothing
    // rather than slivers.
    if mesh.vertices.iter().any(|v| !v.position.is_finite()) {
        return TriangleMesh::default();
    }
    mesh
}

struct Context<'a, 'b> {
    stroke: &'b Stroke<'a>,
    total: f32,
    tile: f32,
}

impl Context<'_, '_> {
    fn half(&self, s: f32) -> f32 {
        let w = (self.stroke.width)(s / self.total);
        if w.is_finite() {
            w.max(0.0) * 0.5
        } else {
            0.0
        }
    }

    fn color(&self, s: f32) -> [f32; 4] {
        (self.stroke.color)(s / self.total)
    }

    /// `u` at length `s`: through the whole line, or through its tile —
    /// `1` at a tile's end when `end` says `s` closes a piece.
    fn u(&self, s: f32, end: bool) -> f32 {
        match self.stroke.texture {
            LineTexture::Stretch => s / self.total,
            LineTexture::Tile => {
                let tiles = s / self.tile;
                let fraction = tiles - tiles.floor();
                if end && fraction < 1e-4 && tiles > 0.5 {
                    1.0
                } else {
                    fraction
                }
            }
        }
    }

    /// The strip from `a` (`s0` along) to `b` (`s1` along), in pieces.
    fn segment(&self, mesh: &mut TriangleMesh, a: Vec2, b: Vec2, s0: f32, s1: f32) {
        let direction = (b - a).normalize();
        let normal = direction.perp();
        let mut cuts = vec![s0, s1];
        if self.stroke.texture == LineTexture::Tile {
            let mut k = (s0 / self.tile).floor() + 1.0;
            while k * self.tile < s1 - 1e-5 && cuts.len() < 4096 {
                cuts.push(k * self.tile);
                k += 1.0;
            }
        }
        let piece = self.total / CURVE_PIECES;
        let mut k = (s0 / piece).floor() + 1.0;
        while k * piece < s1 - 1e-5 && cuts.len() < 4096 {
            cuts.push(k * piece);
            k += 1.0;
        }
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|b, a| (*b - *a).abs() < 1e-5);
        for pair in cuts.windows(2) {
            let (sa, sb) = (pair[0], pair[1]);
            let (pa, pb) = (a + direction * (sa - s0), a + direction * (sb - s0));
            let (ha, hb) = (self.half(sa), self.half(sb));
            let (ua, ub) = (self.u(sa, false), self.u(sb, true));
            let (ca, cb) = (self.color(sa), self.color(sb));
            let l0 = mesh.vertex(pa + normal * ha, [ua, 0.0], ca);
            let r0 = mesh.vertex(pa - normal * ha, [ua, 1.0], ca);
            let l1 = mesh.vertex(pb + normal * hb, [ub, 0.0], cb);
            let r1 = mesh.vertex(pb - normal * hb, [ub, 1.0], cb);
            mesh.triangle(l0, r0, r1);
            mesh.triangle(l0, r1, l1);
        }
    }

    /// The outer side of the corner at `at`, from `into` to `out`, `s` along.
    fn joint(&self, mesh: &mut TriangleMesh, at: Vec2, into: Vec2, out: Vec2, s: f32) {
        let turn = into.perp_dot(out);
        if turn.abs() < 1e-6 && into.dot(out) > 0.0 {
            return;
        }
        // A left turn opens on the right.
        let side = if turn > 0.0 { -1.0 } else { 1.0 };
        let (n0, n1) = (into.perp() * side, out.perp() * side);
        let half = self.half(s);
        let (u, v) = (self.u(s, false), if side > 0.0 { 0.0 } else { 1.0 });
        let color = self.color(s);
        let centre = mesh.vertex(at, [u, 0.5], color);
        let from = mesh.vertex(at + n0 * half, [u, v], color);
        let to = mesh.vertex(at + n1 * half, [u, v], color);
        match self.stroke.joint {
            LineJoint::Bevel => mesh.triangle(centre, from, to),
            LineJoint::Miter => {
                let bisector = (n0 + n1).normalize_or_zero();
                let reach = bisector.dot(n0);
                let limit = self.stroke.miter_limit.max(1.0);
                if reach > 1e-4 && 1.0 / reach <= limit {
                    let tip = mesh.vertex(at + bisector * (half / reach), [u, v], color);
                    mesh.triangle(centre, from, tip);
                    mesh.triangle(centre, tip, to);
                } else {
                    mesh.triangle(centre, from, to);
                }
            }
            LineJoint::Round => {
                let angle = n0.perp_dot(n1).atan2(n0.dot(n1));
                self.fan(mesh, centre, at, n0, angle, half, [u, v], color, from, to);
            }
        }
    }

    /// The cap at the end `at`, facing `outward`, `s` along.
    fn cap(&self, mesh: &mut TriangleMesh, at: Vec2, outward: Vec2, s: f32, cap: LineCap) {
        let half = self.half(s);
        let normal = outward.perp();
        let u = self.u(s, s > 0.0);
        let color = self.color(s);
        match cap {
            LineCap::Butt => {}
            LineCap::Square => {
                // Left of the line is `+perp` of its direction; facing back
                // at the first point, that is `-perp` of `outward`.
                let left = if s > 0.0 { normal } else { -normal };
                let l0 = mesh.vertex(at + left * half, [u, 0.0], color);
                let r0 = mesh.vertex(at - left * half, [u, 1.0], color);
                let l1 = mesh.vertex(at + left * half + outward * half, [u, 0.0], color);
                let r1 = mesh.vertex(at - left * half + outward * half, [u, 1.0], color);
                mesh.triangle(l0, r0, r1);
                mesh.triangle(l0, r1, l1);
            }
            LineCap::Round => {
                // `normal` is the line's left at its last point and its
                // right at its first.
                let (v_from, v_to) = if s > 0.0 { (0.0, 1.0) } else { (1.0, 0.0) };
                let centre = mesh.vertex(at, [u, 0.5], color);
                let from = mesh.vertex(at + normal * half, [u, v_from], color);
                let to = mesh.vertex(at - normal * half, [u, v_to], color);
                self.fan(
                    mesh,
                    centre,
                    at,
                    normal,
                    -PI,
                    half,
                    [u, 0.5],
                    color,
                    from,
                    to,
                );
            }
        }
    }

    /// A fan round `at` from `start` (a unit vector) turning by `angle`,
    /// `half` out, between the existing corners `from` and `to`.
    #[allow(clippy::too_many_arguments)]
    fn fan(
        &self,
        mesh: &mut TriangleMesh,
        centre: u32,
        at: Vec2,
        start: Vec2,
        angle: f32,
        half: f32,
        uv: [f32; 2],
        color: [f32; 4],
        from: u32,
        to: u32,
    ) {
        let steps = (angle.abs() / ROUND_STEP).ceil().clamp(1.0, 64.0) as usize;
        let mut previous = from;
        for i in 1..steps {
            let turned = Vec2::from_angle(angle * i as f32 / steps as f32).rotate(start);
            let next = mesh.vertex(at + turned * half, uv, color);
            mesh.triangle(centre, previous, next);
            previous = next;
        }
        mesh.triangle(centre, previous, to);
    }
}
