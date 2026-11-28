//! The metallic × roughness sphere grid under one directional light: a
//! smooth sphere's highlight a small bright spot, a rough one's dim and
//! wide, in each row.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the picture.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    DirectionalLight, Lighting3D, Mesh3D, StandardSurface, SurfaceTextures, View3D,
};

#[test]
fn a_sphere_grid_sweeps_metallic_and_roughness() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    gpu.renderer.set_msaa(4);
    let ball = Mesh3D::new(&gpu.device, "sphere", &sphere(32, 64)).expect("a sphere");
    const STEPS: usize = 5;
    let mut materials = Vec::new();
    for row in 0..2 {
        for column in 0..STEPS {
            let surface = StandardSurface {
                base_color: [0.95, 0.64, 0.54, 1.0],
                metallic: row as f32,
                roughness: (column as f32 + 0.5) / STEPS as f32,
                ..StandardSurface::DEFAULT
            };
            materials.push(
                gpu.renderer
                    .surface_material(&surface, &SurfaceTextures::NONE),
            );
        }
    }
    let draws: Vec<Draw<'_>> = materials
        .iter()
        .enumerate()
        .map(|(i, material)| {
            let (row, column) = (i / STEPS, i % STEPS);
            Draw {
                mesh: &ball,
                material,
                instances: vec![instance(
                    Vec3::new(column as f32 * 1.2 - 2.4, 0.6 - row as f32 * 1.2, -6.0),
                    Quat::IDENTITY,
                    1.0,
                    WHITE,
                )],
            }
        })
        .collect();
    let (width, height) = (320, 160);
    let view = View3D::perspective(
        Affine3A::IDENTITY,
        25f32.to_radians(),
        width as f32 / height as f32,
        View3D::DEFAULT_NEAR,
        None,
    );
    let lighting = Lighting3D::sun(
        DirectionalLight {
            direction: Vec3::new(-0.4, -0.5, -1.0).normalize(),
            color: [1.0; 3],
            energy: 2.0,
        },
        [0.15; 3],
    );
    let pixels = picture(&mut gpu, (width, height), view, lighting, &draws);
    gpu.renderer.set_msaa(1);
    save("surface-sphere-grid", width, height, &pixels);
    // Each sphere's brightest pixel, in its fifth of the width and half
    // of the height: a smooth sphere's highlight is a small bright spot,
    // a rough one's a dim wide one.
    let peak = |column: usize, row: usize| {
        let band = width / STEPS as u32;
        let mut best = 0u32;
        for y in row as u32 * height / 2..(row as u32 + 1) * height / 2 {
            for x in column as u32 * band..(column as u32 + 1) * band {
                let p = pixels[(y * width + x) as usize];
                best = best.max(u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]));
            }
        }
        best
    };
    for row in 0..2 {
        let peaks: Vec<u32> = (0..STEPS).map(|c| peak(c, row)).collect();
        println!("row {row} peaks {peaks:?}");
        assert!(
            peaks.iter().all(|&p| p > 60),
            "every sphere drawn: {peaks:?}"
        );
        assert!(
            peaks[0] > peaks[STEPS - 1] + 40,
            "smooth brighter at its peak than rough: {peaks:?}"
        );
    }
}
