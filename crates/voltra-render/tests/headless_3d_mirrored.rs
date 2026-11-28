//! The 3D pass on a headless device, mirrored and flattened: a negatively
//! scaled model draws its front faces and still tests depth, and a model
//! flattened to a plane draws at every angle.

mod common;

use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{Instance3D, Mesh3D, MeshDraw3D, Winding};

const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

fn is_blue(p: [u8; 4]) -> bool {
    p[2] > 200 && p[0] < 40 && p[1] < 40
}

fn is_yellow(p: [u8; 4]) -> bool {
    p[0] > 200 && p[1] > 200 && p[2] < 40
}

#[test]
fn a_mirrored_square_and_cube_draw_their_fronts_and_test_depth() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let cube = Mesh3D::new(&gpu.device, "cube", &cube()).expect("a cube");
    let view = looking_down_minus_z(45.0, None);
    let turned = Quat::from_rotation_y(0.6) * Quat::from_rotation_x(0.3);
    let left = Vec3::new(-0.9, 0.0, -4.0);
    let right = Vec3::new(0.9, 0.0, -4.0);
    let mut counts = Vec::new();
    for flip in [1.0, -1.0] {
        let mirror = Vec3::new(flip, 1.0, 1.0);
        let instances = [
            // A red wall behind both, a green square left, a blue cube right
            // with a yellow square in front of its middle.
            scaled(
                Vec3::new(0.0, 0.0, -8.0),
                Quat::IDENTITY,
                Vec3::splat(20.0),
                RED,
            ),
            scaled(left, Quat::IDENTITY, mirror, GREEN),
            scaled(right, turned, mirror * 0.8, BLUE),
            scaled(
                right + Vec3::new(0.0, 0.0, 0.9),
                Quat::IDENTITY,
                Vec3::splat(0.3),
                YELLOW,
            ),
        ];
        let mirrored = if flip < 0.0 {
            Winding::Clockwise
        } else {
            Winding::CounterClockwise
        };
        let draws = [
            MeshDraw3D {
                mesh: &square,
                instances: 0..1,
                winding: Winding::CounterClockwise,
            },
            MeshDraw3D {
                mesh: &square,
                instances: 1..2,
                winding: mirrored,
            },
            MeshDraw3D {
                mesh: &cube,
                instances: 2..3,
                winding: mirrored,
            },
            MeshDraw3D {
                mesh: &square,
                instances: 3..4,
                winding: Winding::CounterClockwise,
            },
        ];
        let pixels = gpu.draw_all((64, 64), view, &instances, &draws);
        let green = pixels.iter().filter(|&&p| is_green(p)).count();
        let blue = pixels.iter().filter(|&&p| is_blue(p)).count();
        let yellow = pixels.iter().filter(|&&p| is_yellow(p)).count();
        let red = pixels.iter().filter(|&&p| is_red(p)).count();
        assert!(
            green > 100 && blue > 100 && yellow > 4,
            "{flip}: {green} {blue} {yellow}"
        );
        // The wall behind is hidden where they stand: depth holds.
        assert_eq!(green + blue + yellow + red, 64 * 64, "{flip}");
        counts.push((green, blue, yellow));
    }
    // A mirrored square and a mirrored symmetric cube cover what the
    // unmirrored ones do, within a pixel row of the cube's turned edges.
    let (plain, flipped) = (counts[0], counts[1]);
    assert_eq!(plain.0, flipped.0, "the square");
    assert!(
        (plain.1 as i32 - flipped.1 as i32).abs() <= 16,
        "{plain:?} {flipped:?}"
    );
    assert_eq!(plain.2, flipped.2, "the yellow square over the cube");
}

#[test]
fn a_model_flattened_along_a_turned_axis_draws_at_every_angle() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let view = looking_down_minus_z(45.0, None);
    // A one-sided square turned about X, flattened along Z, spun in the
    // screen's plane: a rectangle facing the camera, its front towards it or
    // away as the turn passes a quarter, whose determinant is rounding
    // noise. Its winding once followed the noise's sign, and the square
    // vanished at some angles and showed at their neighbours.
    for flatten in [Vec3::new(1.0, 1.0, 0.0), Vec3::new(-1.0, 1.0, 0.0)] {
        let mut counts = Vec::new();
        for step in 0..36 {
            let angle = (step as f32 * 10.0 + 3.0).to_radians();
            let model = Affine3A::from_translation(Vec3::new(0.0, 0.0, -4.0))
                * Affine3A::from_rotation_z(angle)
                * Affine3A::from_scale(flatten)
                * Affine3A::from_rotation_x(0.4 * angle + 0.3);
            // Edge-on near a quarter turn draws a sliver: left out.
            if (0.4 * angle + 0.3).cos().abs() < 0.25 {
                continue;
            }
            let winding = Instance3D::winding(&model);
            assert_eq!(winding, Winding::Flat, "{flatten} at {angle}");
            let instances = [Instance3D::new(model, RED).expect("a plane is drawn")];
            let draws = [MeshDraw3D {
                mesh: &square,
                instances: 0..1,
                winding,
            }];
            let pixels = gpu.draw_all((64, 64), view, &instances, &draws);
            counts.push(pixels.iter().filter(|&&p| is_red(p)).count());
        }
        let (least, most) = (
            *counts.iter().min().expect("36"),
            *counts.iter().max().expect("36"),
        );
        // 1 m wide and |cos| of a metre high at 4 m, some 360 pixels at
        // most: never gone, whichever way its front faces.
        assert!(least > 30, "{flatten}: {counts:?}");
        assert!(most < 64 * 64 / 4, "{flatten}: {counts:?}");
    }
}
