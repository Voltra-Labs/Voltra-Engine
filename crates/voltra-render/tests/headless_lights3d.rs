//! Point, spot and several directional lights, the per-instance light
//! list, the fog and the sky, in pixels: each read back and compared with
//! the CPU twins — `render3d::surface::brdf` for the surface,
//! `PunctualLight::attenuation` for the falloff and the cone,
//! `Fog::amount` and `SkyGradient::color` for the environment.
//!
//! Set `VOLTRA_SHOT_DIR` to keep the pictures.

mod common;

use common::surface::*;
use common::*;
use voltra_render::glam::{Quat, Vec3};
use voltra_render::render3d::surface::brdf;
use voltra_render::render3d::{
    AlphaMode, DirectionalLight, Fog, Instance3D, LightList3D, Lighting3D, Mesh3D, PunctualLight,
    SkyGradient, SpotCone, StandardSurface, SurfaceMaterial3D, SurfaceTextures,
};

/// A rough, light dielectric: what the light tests are lit on.
const MATTE: StandardSurface = StandardSurface {
    base_color: [0.8, 0.8, 0.8, 1.0],
    metallic: 0.0,
    roughness: 0.8,
    ..StandardSurface::DEFAULT
};

fn point(surface: &StandardSurface) -> brdf::Point {
    brdf::Point {
        base: Vec3::new(
            surface.base_color[0],
            surface.base_color[1],
            surface.base_color[2],
        ),
        metallic: surface.metallic,
        roughness: surface.roughness,
        occlusion: 1.0,
        emissive: Vec3::ZERO,
    }
}

/// What the CPU says the wall's pixel `(x, y)` reads under the
/// directional lights of `suns` and the point and spot lights of
/// `lamps`, with `ambient`: each light's direct share through the BRDF,
/// summed.
fn expected_lit(
    surface: &StandardSurface,
    suns: &[DirectionalLight],
    lamps: &[PunctualLight],
    ambient: [f32; 3],
    x: u32,
    y: u32,
) -> [u8; 3] {
    let p = on_wall(x, y);
    let n = Vec3::Z;
    let v = (-p).normalize();
    let point = point(surface);
    let shade = |l: Vec3, color: Vec3| {
        brdf::shade(
            &point,
            &brdf::Light {
                n,
                v,
                l,
                color,
                ambient: Vec3::ZERO,
            },
        )
    };
    let mut sum = brdf::shade(
        &point,
        &brdf::Light {
            n,
            v,
            l: n,
            color: Vec3::ZERO,
            ambient: Vec3::from(ambient),
        },
    );
    for sun in suns {
        sum += shade(
            -sun.direction.normalize(),
            Vec3::from(sun.color) * sun.energy,
        );
    }
    for lamp in lamps {
        let to_light = lamp.position - p;
        let l = if to_light.length_squared() > 1e-30 {
            to_light.normalize()
        } else {
            n
        };
        sum += shade(
            l,
            Vec3::from(lamp.color) * lamp.energy * lamp.attenuation(to_light),
        );
    }
    let sum = sum.min(Vec3::splat(brdf::MAX_LIGHT));
    [encode(sum.x), encode(sum.y), encode(sum.z)]
}

/// The wall through `material`, its instance lit by `list`, under
/// `lighting`.
fn wall_under(
    gpu: &mut Gpu,
    material: &SurfaceMaterial3D,
    lighting: Lighting3D,
    list: LightList3D,
) -> Vec<[u8; 4]> {
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let mut wall = wall();
    wall.set_lights(&list);
    picture(
        gpu,
        (SIZE, SIZE),
        looking_down_minus_z(FOV, None),
        lighting,
        &[Draw {
            mesh: &square,
            material,
            instances: vec![wall],
        }],
    )
}

/// How many of `pixels` are off the CPU's `want` by more than `by`, and
/// the worst.
fn misses(pixels: &[[u8; 4]], by: i32, want: impl Fn(u32, u32) -> [u8; 3]) -> (usize, i32) {
    let mut count = 0;
    let mut worst = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let got = pixels[(y * SIZE + x) as usize];
            let want = want(x, y);
            let off = (0..3)
                .map(|i| (i32::from(got[i]) - i32::from(want[i])).abs())
                .max()
                .unwrap_or(0);
            worst = worst.max(off);
            if off > by {
                count += 1;
            }
        }
    }
    (count, worst)
}

#[test]
fn a_point_light_falls_off_as_the_cpu_says_and_reaches_zero_at_its_range() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    let lamp = PunctualLight {
        position: Vec3::new(0.3, 0.2, -DISTANCE + 1.0),
        color: [1.0, 0.8, 0.6],
        energy: 1.5,
        range: 1.6,
        spot: None,
    };
    let lighting = Lighting3D {
        punctual: vec![lamp],
        ..Lighting3D::DARK
    };
    let pixels = wall_under(&mut gpu, &material, lighting, LightList3D::new(0, &[0]));
    save("lights3d_point", SIZE, SIZE, &pixels);
    let (count, worst) = misses(&pixels, 2, |x, y| {
        expected_lit(&MATTE, &[], &[lamp], [0.0; 3], x, y)
    });
    assert_eq!(
        count, 0,
        "{count} pixels off the CPU by more than 2 (worst {worst})"
    );
    // Past the range: nothing at all; inside it, light.
    let mut dark = 0;
    let mut lit = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = pixels[(y * SIZE + x) as usize];
            let distance = (lamp.position - on_wall(x, y)).length();
            if distance >= lamp.range {
                assert_eq!(&p[..3], &[0, 0, 0], "lit past the range at {x}, {y}");
                dark += 1;
            } else if distance < lamp.range - 0.2 {
                assert!(p[0] > 0, "dark inside the range at {x}, {y}");
                lit += 1;
            }
        }
    }
    assert!(dark > 200 && lit > 1000, "{dark} dark, {lit} lit");
    eprintln!("point light: worst {worst}/255 of the CPU; {dark} pixels past the range");
}

#[test]
fn a_point_light_on_the_surface_is_finite() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    // A millimetre off the wall, over the middle pixel: closer than the
    // centimetre the inverse square stops at.
    let lamp = PunctualLight {
        position: on_wall(32, 32) + Vec3::Z * 0.001,
        color: [1.0; 3],
        energy: 1.0,
        range: 1.0,
        spot: None,
    };
    let pixels = wall_under(
        &mut gpu,
        &material,
        Lighting3D {
            punctual: vec![lamp],
            ..Lighting3D::DARK
        },
        LightList3D::new(0, &[0]),
    );
    // Saturated there, not NaN (which reads back black) nor a hole, and
    // falling away around it.
    let middle = pixels[(32 * SIZE + 32) as usize];
    assert_eq!(&middle[..3], &[255, 255, 255], "{middle:?}");
    let corner = pixels[(8 * SIZE + 8) as usize];
    assert!(corner[0] < 255, "{corner:?}");
}

#[test]
fn a_spot_cone_is_whole_inside_its_inner_angle_and_dark_past_its_outer() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    let (inner, outer) = (10f32, 18f32);
    let spot = PunctualLight {
        position: Vec3::ZERO,
        color: [1.0; 3],
        energy: 6.0,
        range: 10.0,
        spot: Some(SpotCone {
            direction: Vec3::NEG_Z,
            inner: inner.to_radians(),
            outer: outer.to_radians(),
        }),
    };
    let whole = PunctualLight { spot: None, ..spot };
    let pixels = wall_under(
        &mut gpu,
        &material,
        Lighting3D {
            punctual: vec![spot],
            ..Lighting3D::DARK
        },
        LightList3D::new(0, &[0]),
    );
    save("lights3d_spot", SIZE, SIZE, &pixels);
    let (count, worst) = misses(&pixels, 2, |x, y| {
        expected_lit(&MATTE, &[], &[spot], [0.0; 3], x, y)
    });
    assert_eq!(count, 0, "{count} pixels off the CPU (worst {worst})");
    let (mut inside, mut outside) = (0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = pixels[(y * SIZE + x) as usize];
            let angle = on_wall(x, y)
                .normalize()
                .angle_between(Vec3::NEG_Z)
                .to_degrees();
            if angle < inner - 1.0 {
                // The cone takes nothing: lit as the light without one.
                let want = expected_lit(&MATTE, &[], &[whole], [0.0; 3], x, y);
                assert!(
                    within(p, want, 2),
                    "{p:?} at {angle}° inside, want {want:?}"
                );
                inside += 1;
            } else if angle > outer + 1.0 {
                assert!(p[..3].iter().all(|&c| c == 0), "{p:?} at {angle}° outside");
                outside += 1;
            }
        }
    }
    assert!(
        inside > 100 && outside > 1000,
        "{inside} inside, {outside} outside"
    );
}

/// Two suns: a warm one from the upper right, a cool one from the left.
fn two_suns() -> [DirectionalLight; 2] {
    [
        DirectionalLight {
            direction: Vec3::new(-0.6, -0.5, -1.0).normalize(),
            color: [1.0, 0.7, 0.4],
            energy: 1.2,
        },
        DirectionalLight {
            direction: Vec3::new(0.9, 0.1, -0.6).normalize(),
            color: [0.3, 0.5, 1.0],
            energy: 0.8,
        },
    ]
}

#[test]
fn two_suns_add_and_an_instance_s_bits_choose_between_them() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    let suns = two_suns();
    let ambient = [0.02, 0.02, 0.03];
    let lighting = Lighting3D {
        directional: suns.to_vec(),
        ambient,
        ..Lighting3D::DARK
    };
    let both = wall_under(
        &mut gpu,
        &material,
        lighting.clone(),
        LightList3D::default(),
    );
    save("lights3d_two_suns", SIZE, SIZE, &both);
    let (count, worst) = misses(&both, 2, |x, y| {
        expected_lit(&MATTE, &suns, &[], ambient, x, y)
    });
    assert_eq!(count, 0, "both: {count} off (worst {worst})");
    for (bits, sun) in [(0b01, 0), (0b10, 1)] {
        let one = wall_under(
            &mut gpu,
            &material,
            lighting.clone(),
            LightList3D::new(bits, &[]),
        );
        let (count, worst) = misses(&one, 2, |x, y| {
            expected_lit(&MATTE, &suns[sun..=sun], &[], ambient, x, y)
        });
        assert_eq!(count, 0, "sun {sun} alone: {count} off (worst {worst})");
    }
    // Each adds: the pair is brighter than either.
    let middle = both[(32 * SIZE + 32) as usize];
    let warm = expected_lit(&MATTE, &suns[..1], &[], ambient, 32, 32);
    assert!(middle[2] > warm[2] + 20, "{middle:?} over {warm:?}");
}

#[test]
fn an_instance_is_lit_by_the_lights_its_list_names_and_no_other() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    // Nine lights in front of the wall, the ninth bright and central.
    let mut lamps: Vec<PunctualLight> = (0..8)
        .map(|i| {
            let angle = i as f32 / 8.0 * std::f32::consts::TAU;
            PunctualLight {
                position: Vec3::new(angle.cos(), angle.sin(), -DISTANCE + 0.8),
                color: [0.4 + 0.07 * i as f32, 0.5, 0.9 - 0.08 * i as f32],
                energy: 0.3,
                range: 3.0,
                spot: None,
            }
        })
        .collect();
    lamps.push(PunctualLight {
        position: Vec3::new(0.0, 0.0, -DISTANCE + 0.5),
        color: [1.0; 3],
        energy: 5.0,
        range: 3.0,
        spot: None,
    });
    let lighting = Lighting3D {
        punctual: lamps.clone(),
        ..Lighting3D::DARK
    };
    let eight: Vec<u8> = (0..8).collect();
    let pixels = wall_under(
        &mut gpu,
        &material,
        lighting.clone(),
        LightList3D::new(0, &eight),
    );
    save("lights3d_eight_of_nine", SIZE, SIZE, &pixels);
    let (count, worst) = misses(&pixels, 2, |x, y| {
        expected_lit(&MATTE, &[], &lamps[..8], [0.0; 3], x, y)
    });
    assert_eq!(count, 0, "the eight named: {count} off (worst {worst})");
    // An index past the frame's lights lights nothing; the ninth alone
    // lights as itself.
    let past = wall_under(
        &mut gpu,
        &material,
        lighting.clone(),
        LightList3D::new(0, &[100]),
    );
    assert!(
        past.iter().all(|p| p[..3] == [0, 0, 0]),
        "an index past 9 lit"
    );
    let ninth = wall_under(&mut gpu, &material, lighting, LightList3D::new(0, &[8]));
    let (count, _) = misses(&ninth, 2, |x, y| {
        expected_lit(&MATTE, &[], &lamps[8..], [0.0; 3], x, y)
    });
    assert_eq!(count, 0);
}

/// An unlit white surface: what the fog tests fade.
fn white_unlit(gpu: &mut Gpu) -> SurfaceMaterial3D {
    unlit(&mut gpu.renderer)
}

#[test]
fn fog_fades_by_distance_and_height_as_the_cpu_says() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = white_unlit(&mut gpu);
    let fog = Fog {
        color: [0.2, 0.4, 0.6],
        density: 0.25,
        height: -0.5,
        height_falloff: 0.8,
        sky_tint: 0.0,
    };
    let pixels = wall_under(
        &mut gpu,
        &material,
        Lighting3D {
            fog: Some(fog),
            ..Lighting3D::DARK
        },
        LightList3D::default(),
    );
    save("lights3d_fog_wall", SIZE, SIZE, &pixels);
    let fogged = |x: u32, y: u32| {
        let amount = fog.amount(Vec3::ZERO, on_wall(x, y));
        let c = Vec3::ONE.lerp(Vec3::from(fog.color), amount);
        [encode(c.x), encode(c.y), encode(c.z)]
    };
    let (count, worst) = misses(&pixels, 2, fogged);
    assert_eq!(count, 0, "{count} off the CPU's fog (worst {worst})");
    // Thicker below: the bottom row is nearer the fog's colour.
    let (top, bottom) = (pixels[32], pixels[((SIZE - 1) * SIZE + 32) as usize]);
    assert!(bottom[0] < top[0], "{bottom:?} below {top:?}");

    // Plain distance fog at known depths: squares at 2, 5, 10 and 20 m.
    let distance_fog = Fog {
        height_falloff: 0.0,
        density: 0.08,
        ..fog
    };
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let depths = [2.0f32, 5.0, 10.0, 20.0];
    let instances: Vec<Instance3D> = depths
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            // Side by side, each filling a quarter of the view's width.
            let x = (i as f32 - 1.5) * 0.25;
            let at = Vec3::new(x, 0.0, -1.0) * d;
            instance(at, Quat::IDENTITY, 0.2 * d, WHITE)
        })
        .collect();
    let pixels = picture(
        &mut gpu,
        (SIZE, SIZE),
        looking_down_minus_z(FOV, None),
        Lighting3D {
            fog: Some(distance_fog),
            ..Lighting3D::DARK
        },
        &[Draw {
            mesh: &square,
            material: &material,
            instances,
        }],
    );
    save("lights3d_fog_depths", SIZE, SIZE, &pixels);
    let tan = (FOV.to_radians() * 0.5).tan();
    for (i, &d) in depths.iter().enumerate() {
        // The square's centre on screen.
        let nx = (i as f32 - 1.5) * 0.25 / tan;
        let x = ((nx * 0.5 + 0.5) * SIZE as f32) as u32;
        let p = pixels[(32 * SIZE + x) as usize];
        let point = Vec3::new((i as f32 - 1.5) * 0.25, 0.0, -1.0) * d;
        let amount = distance_fog.amount(Vec3::ZERO, point);
        let c = Vec3::ONE.lerp(Vec3::from(distance_fog.color), amount);
        let want = [encode(c.x), encode(c.y), encode(c.z)];
        assert!(within(p, want, 2), "{d} m: {p:?}, want {want:?} ({amount})");
    }
}

/// A sky of three distinct colours.
const SKY: SkyGradient = SkyGradient {
    top: [0.05, 0.15, 0.6],
    horizon: [0.7, 0.75, 0.8],
    bottom: [0.25, 0.2, 0.1],
};

fn sky_at(x: u32, y: u32) -> [u8; 3] {
    let c = SKY.color(on_wall(x, y).normalize());
    [encode(c.x), encode(c.y), encode(c.z)]
}

#[test]
fn the_sky_is_drawn_behind_the_geometry_only_and_under_what_blends() {
    for samples in [1, 4] {
        let Some(mut gpu) = gpu() else {
            eprintln!("no adapter: skipping");
            return;
        };
        gpu.renderer.set_msaa(samples);
        let red = unlit(&mut gpu.renderer);
        let glass = gpu.renderer.surface_material(
            &StandardSurface {
                alpha: AlphaMode::Blend,
                ..StandardSurface::UNLIT
            },
            &SurfaceTextures::NONE,
        );
        let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
        // A red square in the middle, a half-clear white one lower left.
        let middle = instance(Vec3::new(0.0, 0.0, -DISTANCE), Quat::IDENTITY, 0.8, RED);
        let pane = instance(
            Vec3::new(-0.7, -0.7, -DISTANCE),
            Quat::IDENTITY,
            0.6,
            [1.0, 1.0, 1.0, 0.5],
        );
        let pixels = picture(
            &mut gpu,
            (SIZE, SIZE),
            looking_down_minus_z(FOV, None),
            Lighting3D {
                sky: Some(SKY),
                ..Lighting3D::DARK
            },
            &[
                Draw {
                    mesh: &square,
                    material: &red,
                    instances: vec![middle],
                },
                Draw {
                    mesh: &square,
                    material: &glass,
                    instances: vec![pane],
                },
            ],
        );
        save(&format!("lights3d_sky_{samples}x"), SIZE, SIZE, &pixels);
        // Whether pixel (x, y) lies on a square of `half` metres about `at`.
        let on = |x: u32, y: u32, at: Vec3, half: f32, margin: f32| {
            let p = on_wall(x, y);
            (p.x - at.x).abs() < half - margin && (p.y - at.y).abs() < half - margin
        };
        let (mut sky, mut wall, mut blended) = (0, 0, 0);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let p = pixels[(y * SIZE + x) as usize];
                let near_edge =
                    on(x, y, middle_at(), 0.4, -0.08) && !on(x, y, middle_at(), 0.4, 0.08);
                let near_pane = on(x, y, pane_at(), 0.3, -0.08);
                if on(x, y, middle_at(), 0.4, 0.08) {
                    assert!(is_red(p), "{p:?} on the square at {x}, {y}");
                    wall += 1;
                } else if on(x, y, pane_at(), 0.3, 0.08) {
                    // Half white over the sky: blended after it.
                    let s = sky_at(x, y).map(decode);
                    let mix = s.map(|c| c * 0.5 + 0.5);
                    let want = [encode(mix[0]), encode(mix[1]), encode(mix[2])];
                    assert!(within(p, want, 2), "{p:?} on the pane, want {want:?}");
                    blended += 1;
                } else if !near_edge && !near_pane {
                    let want = sky_at(x, y);
                    assert!(within(p, want, 2), "{p:?} at {x}, {y}: sky {want:?}");
                    sky += 1;
                }
            }
        }
        assert!(
            sky > 2000 && wall > 150 && blended > 80,
            "{sky} {wall} {blended}"
        );
    }
}

fn middle_at() -> Vec3 {
    Vec3::new(0.0, 0.0, -DISTANCE)
}

fn pane_at() -> Vec3 {
    Vec3::new(-0.7, -0.7, -DISTANCE)
}

#[test]
fn fog_tinted_by_the_sky_fades_distant_things_into_it() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = white_unlit(&mut gpu);
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    let fog = Fog {
        color: [1.0, 0.0, 1.0],
        density: 0.06,
        height: 0.0,
        height_falloff: 0.0,
        sky_tint: 1.0,
    };
    // A near square on the left, a far one on the right, both a quarter
    // of the view across.
    let near = instance(Vec3::new(-0.5, 0.0, -2.0), Quat::IDENTITY, 0.5, WHITE);
    let far = instance(Vec3::new(20.0, 0.0, -80.0), Quat::IDENTITY, 20.0, WHITE);
    let pixels = picture(
        &mut gpu,
        (SIZE, SIZE),
        looking_down_minus_z(FOV, None),
        Lighting3D {
            sky: Some(SKY),
            fog: Some(fog),
            ..Lighting3D::DARK
        },
        &[Draw {
            mesh: &square,
            material: &material,
            instances: vec![near, far],
        }],
    );
    save("lights3d_fog_into_sky", SIZE, SIZE, &pixels);
    let pixel_of = |p: Vec3| {
        let tan = (FOV.to_radians() * 0.5).tan();
        let ndc = (p.truncate() / -p.z) / tan;
        let x = ((ndc.x * 0.5 + 0.5) * SIZE as f32) as u32;
        let y = ((0.5 - ndc.y * 0.5) * SIZE as f32) as u32;
        (x, y)
    };
    // The far square: within 3/255 of the sky behind it, no magenta.
    let (x, y) = pixel_of(Vec3::new(20.0, 0.0, -80.0));
    let p = pixels[(y * SIZE + x) as usize];
    let want = sky_at(x, y);
    assert!(within(p, want, 3), "far {p:?}, sky {want:?}");
    // The near one: between white and the sky, as the CPU says.
    let (x, y) = pixel_of(Vec3::new(-0.5, 0.0, -2.0));
    let p = pixels[(y * SIZE + x) as usize];
    let amount = fog.amount(Vec3::ZERO, Vec3::new(-0.5, 0.0, -2.0));
    let sky = SKY.color(Vec3::new(-0.5, 0.0, -2.0).normalize());
    let c = Vec3::ONE.lerp(sky, amount);
    let want = [encode(c.x), encode(c.y), encode(c.z)];
    assert!(within(p, want, 2), "near {p:?}, want {want:?}");
    assert!(p[0] > 240, "near stays nearly white: {p:?}");
}

#[test]
fn every_light_kind_at_once_stays_within_the_budget_and_draws() {
    let Some(mut gpu) = gpu() else {
        eprintln!("no adapter: skipping");
        return;
    };
    let material = gpu
        .renderer
        .surface_material(&MATTE, &SurfaceTextures::NONE);
    // 128 lights and four suns: the full array.
    let lamps: Vec<PunctualLight> = (0..128)
        .map(|i| PunctualLight {
            position: Vec3::new(
                (i % 16) as f32 * 0.2 - 1.5,
                (i / 16) as f32 * 0.3 - 1.0,
                -2.5,
            ),
            color: [1.0; 3],
            energy: 0.05,
            range: 1.0,
            spot: None,
        })
        .collect();
    let suns = [two_suns(), two_suns()].concat();
    let picked: Vec<u8> = vec![120, 3, 64, 127, 0, 15, 77, 99];
    let named: Vec<PunctualLight> = picked.iter().map(|&i| lamps[usize::from(i)]).collect();
    let pixels = wall_under(
        &mut gpu,
        &material,
        Lighting3D {
            directional: suns.clone(),
            punctual: lamps,
            ..Lighting3D::DARK
        },
        LightList3D::new(0b1111, &picked),
    );
    let (count, worst) = misses(&pixels, 2, |x, y| {
        expected_lit(&MATTE, &suns, &named, [0.0; 3], x, y)
    });
    assert_eq!(count, 0, "{count} off (worst {worst})");
}
