//! Particles 2 (stage 99), drawn both ways: every feature — the size, speed
//! and alpha curves, the gradient, each emission shape, rotation and spin,
//! and a flipbook — screenshotted headless with the emitter simulated on the
//! CPU and again on the GPU, and the two shots compared.
//!
//! A deterministic feature (one particle, no spread) must match texel for
//! texel, give or take rasterisation; a shape, whose newborns are placed by
//! each side's own generator, must cover the same region about as densely.
//! Each skips without a GPU adapter, or one without compute shaders.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_core::{App, Headless, Screenshot, ScreenshotError, WindowConfig};
use voltra_render::glam::Vec2;
use voltra_scene::particles::{Flipbook, FlipbookTiming, Simulation};
use voltra_scene::{Camera2D, Collider2D, Curve, CurveKey, Gradient, ParticleEmitter, Transform2D};

/// 64 × 64 pixels over four units: sixteen pixels a unit, the origin at
/// pixel (32, 32).
const SHOT: u32 = 64;
const PER_UNIT: f32 = 16.0;
const FRAME: f32 = 1.0 / 60.0;

/// `emitter` run for `frames` on the CPU or the GPU and shot after each one
/// — a GPU emitter is dispatched once per drawn frame, as in the window.
/// `None` without an adapter that can run GPU particles.
fn shoot(
    root: &Path,
    emitter: &ParticleEmitter,
    collider: Option<Collider2D>,
    simulation: Simulation,
    frames: u32,
) -> Option<Screenshot> {
    let mut app = App::new(WindowConfig::default()).with_asset_root(root.to_path_buf());
    // Into the world before the session starts, so it resolves the
    // flipbook's atlas as it does a loaded scene's.
    let world = &mut app.world;
    let camera = world.spawn();
    world.insert(camera, Transform2D::default());
    world.insert(camera, Camera2D::new(2.0));
    let entity = world.spawn();
    world.insert(entity, Transform2D::default());
    if let Some(collider) = collider {
        world.insert(entity, collider);
    }
    world.insert(
        entity,
        ParticleEmitter {
            simulation,
            ..emitter.clone()
        },
    );
    let mut session = Headless::new(app);
    // The first shot makes the device, so the steps after it know a GPU
    // emitter can run there.
    match session.screenshot(SHOT, SHOT) {
        Ok(_) => {}
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            return None;
        }
        Err(e) => panic!("{e}"),
    }
    let mut shot = None;
    for _ in 0..frames {
        session.step(1, FRAME);
        shot = Some(session.screenshot(SHOT, SHOT).expect("a shot"));
    }
    let on_gpu = session
        .world()
        .query::<ParticleEmitter>()
        .any(|(_, emitter)| emitter.gpu_capacity().is_some());
    if simulation == Simulation::Gpu && !on_gpu {
        eprintln!("the adapter cannot run GPU particles; skipping");
        return None;
    }
    shot
}

/// Both shots, or `None` to skip.
fn both(
    root: &Path,
    emitter: &ParticleEmitter,
    collider: Option<Collider2D>,
    frames: u32,
) -> Option<(Screenshot, Screenshot)> {
    let cpu = shoot(root, emitter, collider, Simulation::Cpu, frames)?;
    let gpu = shoot(root, emitter, collider, Simulation::Gpu, frames)?;
    Some((cpu, gpu))
}

fn root() -> voltra_testkit::Scratch {
    voltra_testkit::scratch_root()
}

/// One particle at the origin that does not move, living two seconds.
fn still() -> ParticleEmitter {
    let mut emitter = ParticleEmitter {
        emitting: false,
        lifetime: (2.0, 2.0),
        speed: (0.0, 0.0),
        spread: 0.0,
        direction: 0.0,
        acceleration: Vec2::ZERO,
        size: (1.0, 1.0),
        color_start: [1.0; 4],
        color_end: [1.0; 4],
        ..ParticleEmitter::default()
    };
    emitter.burst(1);
    emitter
}

/// Pixels where the two shots differ by more than a little in any channel.
fn differing(a: &Screenshot, b: &Screenshot) -> usize {
    a.rgba
        .chunks(4)
        .zip(b.rgba.chunks(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 24))
        .count()
}

/// The two shots are the same picture: at most a pixel's edge apart.
fn assert_same(cpu: &Screenshot, gpu: &Screenshot, what: &str) {
    let off = differing(cpu, gpu);
    assert!(off <= 24, "{what}: {off} pixels differ between CPU and GPU");
}

/// Whether a pixel is drawn over: not the clear colour, which the corner
/// shows — every shape here stays well inside it.
fn lit(shot: &Screenshot, x: u32, y: u32) -> bool {
    let clear = shot.pixel(0, 0).expect("a corner");
    shot.pixel(x, y).is_some_and(|p| {
        p[..3]
            .iter()
            .zip(&clear[..3])
            .any(|(a, b)| a.abs_diff(*b) > 60)
    })
}

/// How many pixels are lit, and their box `(min, max)`.
fn coverage(shot: &Screenshot) -> (usize, (u32, u32), (u32, u32)) {
    let mut count = 0;
    let (mut lo, mut hi) = ((u32::MAX, u32::MAX), (0, 0));
    for y in 0..SHOT {
        for x in 0..SHOT {
            if lit(shot, x, y) {
                count += 1;
                lo = (lo.0.min(x), lo.1.min(y));
                hi = (hi.0.max(x), hi.1.max(y));
            }
        }
    }
    (count, lo, hi)
}

/// Lit pixels across the middle row.
fn width(shot: &Screenshot) -> u32 {
    (0..SHOT).filter(|&x| lit(shot, x, SHOT / 2)).count() as u32
}

#[test]
fn a_size_curve_draws_the_same_size_both_ways() {
    let emitter = ParticleEmitter {
        // Two units wide at birth, a quarter of that at death: half-way at
        // one second, a unit and a quarter — twenty pixels.
        size: (2.0, 2.0),
        size_curve: Some(Curve::linear(1.0, 0.25)),
        ..still()
    };
    let Some((cpu, gpu)) = both(&root(), &emitter, None, 60) else {
        return;
    };
    assert_same(&cpu, &gpu, "size curve");
    let expected = (2.0 * (1.0 - 0.75 * 0.5) * PER_UNIT) as i32;
    for shot in [&cpu, &gpu] {
        let w = width(shot) as i32;
        assert!((w - expected).abs() <= 2, "{w} px, not {expected}");
    }
}

#[test]
fn a_speed_curve_slows_the_same_way_both_ways() {
    let emitter = ParticleEmitter {
        speed: (1.0, 1.0),
        size: (0.25, 0.25),
        // Full speed to a stop over the life: after one second of two, at
        // ¾ of a unit rather than one.
        speed_curve: Some(Curve::linear(1.0, 0.0)),
        ..still()
    };
    let Some((cpu, gpu)) = both(&root(), &emitter, None, 60) else {
        return;
    };
    assert_same(&cpu, &gpu, "speed curve");
    for shot in [&cpu, &gpu] {
        let (_, lo, hi) = coverage(shot);
        let centre = (lo.0 + hi.0) as f32 / 2.0 - 32.0;
        assert!(
            (centre / PER_UNIT - 0.75).abs() < 0.1,
            "at {} units",
            centre / PER_UNIT
        );
    }
}

#[test]
fn an_alpha_curve_and_a_gradient_colour_the_same_both_ways() {
    let emitter = ParticleEmitter {
        gradient: Some(Gradient::linear([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0])),
        alpha_curve: Some(Curve::new(vec![
            CurveKey::new(0.0, 1.0),
            CurveKey::new(0.5, 0.5),
        ])),
        ..still()
    };
    let Some((cpu, gpu)) = both(&root(), &emitter, None, 60) else {
        return;
    };
    assert_same(&cpu, &gpu, "gradient and alpha");
    for shot in [&cpu, &gpu] {
        // Half-way from red to blue, at half alpha over the clear colour.
        let p = shot.pixel(32, 32).expect("the middle");
        let clear = shot.pixel(0, 0).expect("a corner");
        assert!(p[0] > 100 && p[2] > 100 && p[1] <= clear[1], "{p:?}");
        assert!(p[0] < 200 && p[2] < 200, "half alpha: {p:?}");
    }
}

#[test]
fn rotation_and_spin_turn_the_same_both_ways() {
    // Born at 30°, turning 15° a second: at 45° after one second, a
    // diamond whose corners reach √2 / 2 of a unit out.
    let emitter = ParticleEmitter {
        rotation: (30f32.to_radians(), 30f32.to_radians()),
        angular_velocity: (15f32.to_radians(), 15f32.to_radians()),
        ..still()
    };
    let Some((cpu, gpu)) = both(&root(), &emitter, None, 60) else {
        return;
    };
    assert_same(&cpu, &gpu, "rotation");
    let expected = (std::f32::consts::SQRT_2 * PER_UNIT) as i32;
    for shot in [&cpu, &gpu] {
        let w = width(shot) as i32;
        assert!((w - expected).abs() <= 2, "{w} px across, not {expected}");
        assert!(!lit(shot, 32 - 7, 32 - 7), "a diamond's corner is empty");
    }
}

/// Many still, small particles over `shape`: the same region, about as
/// densely, both ways, and nothing outside `inside`.
fn assert_shape(
    shape: voltra_scene::particles::EmitShape,
    collider: Option<Collider2D>,
    min_lit: usize,
    inside: impl Fn(Vec2) -> bool,
) {
    let mut emitter = ParticleEmitter {
        shape,
        size: (0.125, 0.125),
        max_particles: 600,
        ..still()
    };
    emitter.burst(599);
    let Some((cpu, gpu)) = both(&root(), &emitter, collider, 2) else {
        return;
    };
    let (cpu_count, cpu_lo, cpu_hi) = coverage(&cpu);
    let (gpu_count, gpu_lo, gpu_hi) = coverage(&gpu);
    assert!(cpu_count >= min_lit, "{shape:?}: CPU drew {cpu_count}");
    let ratio = gpu_count as f32 / cpu_count as f32;
    assert!(
        (0.75..1.33).contains(&ratio),
        "{shape:?}: {cpu_count} pixels on the CPU, {gpu_count} on the GPU"
    );
    for (a, b) in [
        (cpu_lo.0, gpu_lo.0),
        (cpu_lo.1, gpu_lo.1),
        (cpu_hi.0, gpu_hi.0),
        (cpu_hi.1, gpu_hi.1),
    ] {
        assert!(
            a.abs_diff(b) <= 3,
            "{shape:?}: boxes {cpu_lo:?}..{cpu_hi:?} and {gpu_lo:?}..{gpu_hi:?}"
        );
    }
    for shot in [&cpu, &gpu] {
        for y in 0..SHOT {
            for x in 0..SHOT {
                let at = Vec2::new(x as f32 + 0.5 - 32.0, 32.0 - (y as f32 + 0.5)) / PER_UNIT;
                assert!(
                    !lit(shot, x, y) || inside(at),
                    "{shape:?}: lit at {at} outside the shape"
                );
            }
        }
    }
}

/// A pixel and a particle's half-size of slack round a shape's edge.
const SLACK: f32 = 0.15;

#[test]
fn each_emission_shape_covers_the_same_region_both_ways() {
    use voltra_scene::particles::EmitShape;
    assert_shape(EmitShape::Circle { radius: 1.0 }, None, 200, |p| {
        p.length() <= 1.0 + SLACK
    });
    assert_shape(
        EmitShape::Ring {
            inner: 1.0,
            outer: 1.25,
        },
        None,
        200,
        |p| (1.0 - SLACK..=1.25 + SLACK).contains(&p.length()),
    );
    assert_shape(
        EmitShape::Box {
            half_extents: Vec2::new(1.5, 0.5),
        },
        None,
        200,
        |p| p.x.abs() <= 1.5 + SLACK && p.y.abs() <= 0.5 + SLACK,
    );
    assert_shape(EmitShape::Edge { half_length: 1.5 }, None, 60, |p| {
        p.x.abs() <= 1.5 + SLACK && p.y.abs() <= SLACK
    });
    assert_shape(
        EmitShape::Outline,
        Some(Collider2D::Box {
            half_extents: Vec2::new(1.0, 0.75),
        }),
        100,
        |p| {
            let on_x = (p.x.abs() - 1.0).abs() <= SLACK && p.y.abs() <= 0.75 + SLACK;
            let on_y = (p.y.abs() - 0.75).abs() <= SLACK && p.x.abs() <= 1.0 + SLACK;
            on_x || on_y
        },
    );
    // A point: everything in one place.
    assert_shape(EmitShape::Point, None, 1, |p| p.length() <= SLACK);
}

/// A sheet of two 8 × 8 frames, red then green, and its slicing.
fn flipbook_root() -> voltra_testkit::Scratch {
    let root = root();
    let mut sheet = image::RgbaImage::new(16, 8);
    for (x, _, pixel) in sheet.enumerate_pixels_mut() {
        *pixel = if x < 8 {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 255, 0, 255])
        };
    }
    sheet.save(root.join("sheet.png")).expect("the sheet");
    std::fs::write(
        root.join("sheet.atlas.ron"),
        "(version: 1, grid: Some((cell: (8, 8), columns: 2, rows: 1)))",
    )
    .expect("the slicing");
    root
}

#[test]
fn a_flipbook_shows_the_same_frame_both_ways() {
    let root = flipbook_root();
    let flipbook = |timing| ParticleEmitter {
        texture: Some(AssetPath::new("sheet.png").expect("a path")),
        flipbook: Some(Flipbook {
            timing,
            ..Flipbook::new(AssetPath::new("sheet.atlas.ron").expect("a path"))
        }),
        ..still()
    };
    // Over a life of two seconds, once through: the second frame from the
    // first second on. At four frames a second: the second frame from a
    // quarter of a second, the first again from half a second.
    for (timing, frames, green) in [
        (FlipbookTiming::OverLife { cycles: 1.0 }, 30, false),
        (FlipbookTiming::OverLife { cycles: 1.0 }, 70, true),
        (FlipbookTiming::PerSecond { fps: 4.0 }, 25, true),
        (FlipbookTiming::PerSecond { fps: 4.0 }, 40, false),
    ] {
        let Some((cpu, gpu)) = both(&root, &flipbook(timing), None, frames) else {
            return;
        };
        assert_same(&cpu, &gpu, "flipbook");
        for shot in [&cpu, &gpu] {
            let p = shot.pixel(32, 32).expect("the middle");
            let (want, other) = if green { (p[1], p[0]) } else { (p[0], p[1]) };
            assert!(want > 200 && other < 40, "{timing:?} after {frames}: {p:?}");
        }
    }
}
