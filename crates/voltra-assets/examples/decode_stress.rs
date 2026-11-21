//! Times opening a scene's worth of textures: loaded one by one, then
//! preloaded with the decoding across a job pool.
//!
//! `cargo run --release -p voltra-assets --example decode_stress`
//!
//! Writes 48 noisy 512 × 512 PNGs to a scratch folder — noise, because a
//! flat image inflates in no time and would flatter both — and loads them
//! into fresh stores both ways. Not a test: the counts the tests pin are.

use std::time::Instant;

use image::ImageEncoder;
use voltra_assets::{AssetPath, Textures};
use voltra_jobs::JobPool;
use voltra_render::texture;

const COUNT: usize = 48;
const SIZE: u32 = 512;

fn main() {
    let Some((device, queue)) = voltra_testkit::headless_device() else {
        log::error!("no GPU adapter");
        return;
    };
    let root = voltra_testkit::scratch_root();
    let mut seed = 0x9e37_79b9_u32;
    let mut paths = Vec::new();
    for i in 0..COUNT {
        let pixels: Vec<u8> = (0..SIZE * SIZE * 4)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed >> 24) as u8
            })
            .collect();
        let name = format!("sheet{i}.png");
        let file = std::fs::File::create(root.join(&name)).expect("scratch file");
        image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
            .write_image(&pixels, SIZE, SIZE, image::ExtendedColorType::Rgba8)
            .expect("encodes");
        paths.push(AssetPath::new(&name).expect("valid"));
    }
    let layout = texture::bind_group_layout(&device);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let start = Instant::now();
    for path in &paths {
        textures.load(&device, &queue, path);
    }
    let alone = start.elapsed().as_secs_f64() * 1000.0;

    let pool = JobPool::with_available_parallelism();
    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let start = Instant::now();
    textures.preload(&device, &queue, paths.iter().cloned(), &pool);
    let pooled = start.elapsed().as_secs_f64() * 1000.0;

    // An example's report, as `batch_stress` prints its own.
    #[allow(clippy::print_stdout)]
    {
        println!("{COUNT} textures of {SIZE}²: one by one {alone:7.1} ms");
        println!("  preloaded on {} threads {pooled:7.1} ms", pool.threads());
    }
}
