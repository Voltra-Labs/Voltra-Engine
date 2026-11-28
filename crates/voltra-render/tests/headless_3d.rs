//! The 3D pass on a headless device: depth decides what is in front,
//! reversed Z resolves far surfaces, the depth target follows the colour
//! target, and an orthographic view frames what it says it frames.

mod common;

use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{Mesh3D, MeshDraw3D, Pass3D, Scene3D, View3D};
use voltra_render::{read_rgba8, wgpu, Filter, RenderTarget};

#[test]
fn depth_decides_what_is_in_front_whatever_the_submission_order() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let mesh = Mesh3D::new(&gpu.device, "cube", &cube()).expect("a cube");
    let turned = Quat::from_rotation_y(0.5) * Quat::from_rotation_x(0.4);
    // Red in front and to the left, green behind and to the right, the two
    // overlapping on screen and interpenetrating in depth.
    let near = instance(Vec3::new(-0.3, 0.0, -4.0), turned, 1.6, RED);
    let far = instance(Vec3::new(0.5, 0.2, -4.9), turned, 1.6, GREEN);
    let view = looking_down_minus_z(45.0, None);
    let near_first = gpu.draw((64, 64), view, &mesh, &[near, far], &[(0, 1), (1, 2)]);
    let far_first = gpu.draw((64, 64), view, &mesh, &[far, near], &[(0, 1), (1, 2)]);
    // One draw of both, the instanced way, in either order too.
    let both = gpu.draw((64, 64), view, &mesh, &[far, near], &[(0, 2)]);
    assert_eq!(near_first, far_first, "depth, not order, decides");
    assert_eq!(near_first, both);
    let red = near_first.iter().filter(|&&p| is_red(p)).count();
    let green = near_first.iter().filter(|&&p| is_green(p)).count();
    assert!(red > 400 && green > 200, "red {red}, green {green}");
    // The centre is where they overlap: the nearer one's.
    assert!(
        is_red(near_first[32 * 64 + 28]),
        "{:?}",
        near_first[32 * 64 + 28]
    );
}

#[test]
fn reversed_z_resolves_two_planes_a_centimetre_apart_at_500_m() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let mesh = Mesh3D::new(&gpu.device, "square", &square(60.0)).expect("a square");
    // Tilted 50° away so the depth varies across every pixel, as a ground
    // plane's does; the green one 1 cm in front of the red along their
    // shared normal.
    let tilt = Quat::from_rotation_x(-50f32.to_radians());
    let normal = tilt * Vec3::Z;
    let at = Vec3::new(0.0, 0.0, -500.0);
    let back = instance(at, tilt, 1.0, RED);
    let front = instance(at + normal * 0.01, tilt, 1.0, GREEN);
    let view = looking_down_minus_z(6.0, None);
    for order in [[back, front], [front, back]] {
        let pixels = gpu.draw((64, 64), view, &mesh, &order, &[(0, 1), (1, 2)]);
        let green = pixels.iter().filter(|&&p| is_green(p)).count();
        let red = pixels.iter().filter(|&&p| is_red(p)).count();
        assert!(green > 64 * 64 / 2, "the plane fills the view: {green}");
        assert_eq!(red, 0, "the far plane never shows through: {red} pixels");
    }
}

#[test]
fn the_depth_target_follows_the_colour_target_size() {
    let Some(gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let format = gpu.renderer.context().format();
    let mesh = Mesh3D::new(&gpu.device, "cube", &cube()).expect("a cube");
    let mut pass = Pass3D::new(&gpu.device, format);
    assert_eq!(pass.depth_size(), None);
    let instances = [instance(
        Vec3::new(0.0, 0.0, -3.0),
        Quat::IDENTITY,
        1.0,
        RED,
    )];
    let draws = [MeshDraw3D {
        mesh: &mesh,
        instances: 0..1,
        mirrored: false,
    }];
    for (width, height) in [(64, 64), (96, 48), (17, 130)] {
        let target = RenderTarget::new(&gpu.device, "t", format, width, height, Filter::Linear);
        let scene = Scene3D {
            view: View3D::perspective(
                Affine3A::IDENTITY,
                1.0,
                width as f32 / height as f32,
                0.1,
                None,
            ),
            instances: &instances,
            draws: &draws,
            clear: CLEAR,
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        pass.record(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            target.view(),
            (width, height),
            &scene,
        );
        gpu.queue.submit([encoder.finish()]);
        assert_eq!(pass.depth_size(), Some((width, height)));
        let pixels = read_rgba8(&gpu.device, &gpu.queue, target.texture(), width, height);
        let at = |x: u32, y: u32| {
            let i = ((y * width + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };
        assert!(
            is_red(at(width / 2, height / 2)),
            "the cube is in the middle"
        );
        assert!(!is_red(at(0, 0)), "and not in the corner");
    }
}

#[test]
fn an_orthographic_view_frames_its_size_in_metres() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let mesh = Mesh3D::new(&gpu.device, "cube", &cube()).expect("a cube");
    // 2 m above and below the centre on 64 pixels: a 1 m cube face-on is
    // 16 by 16 pixels whatever its distance.
    let view = View3D::orthographic(Affine3A::IDENTITY, 2.0, 1.0, 0.1, 100.0);
    for distance in [3.0, 60.0] {
        let cube = instance(Vec3::new(0.0, 0.0, -distance), Quat::IDENTITY, 1.0, RED);
        let pixels = gpu.draw((64, 64), view, &mesh, &[cube], &[(0, 1)]);
        let red = pixels.iter().filter(|&&p| is_red(p)).count();
        assert!(
            (red as i32 - 256).abs() <= 16,
            "{red} pixels at {distance} m"
        );
    }
    // Past the far plane it is gone.
    let gone = instance(Vec3::new(0.0, 0.0, -101.0), Quat::IDENTITY, 1.0, RED);
    let pixels = gpu.draw((64, 64), view, &mesh, &[gone], &[(0, 1)]);
    assert_eq!(pixels.iter().filter(|&&p| is_red(p)).count(), 0);
}

#[test]
fn nothing_to_draw_clears_and_a_bad_range_is_skipped() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let mesh = Mesh3D::new(&gpu.device, "cube", &cube()).expect("a cube");
    let view = looking_down_minus_z(60.0, None);
    let empty = gpu.draw((16, 16), view, &mesh, &[], &[]);
    assert!(empty.iter().all(|p| p[..3] == [0, 0, 0]));
    let one = [instance(
        Vec3::new(0.0, 0.0, -3.0),
        Quat::IDENTITY,
        1.0,
        RED,
    )];
    // A range past the instances (a draw-list bug) is logged and skipped,
    // not a GPU validation error.
    let pixels = gpu.draw((16, 16), view, &mesh, &one, &[(0, 1), (1, 5)]);
    assert!(is_red(pixels[8 * 16 + 8]));
}
