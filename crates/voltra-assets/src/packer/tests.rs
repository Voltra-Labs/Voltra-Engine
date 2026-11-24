use image::Rgba;
use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::*;
use std::path::Path;

use crate::atlas::AtlasFile;
use crate::path::AssetPath;

/// A `width` by `height` picture, every pixel a colour of its own, inside a
/// transparent border `border` texels wide.
fn picture(name: &str, width: u32, height: u32, border: u32) -> Picture {
    let seed = name
        .bytes()
        .fold(7u8, |a, b| a.wrapping_mul(31).wrapping_add(b));
    let image = RgbaImage::from_fn(width, height, |x, y| {
        let inside = x >= border && y >= border && x + border < width && y + border < height;
        if inside {
            Rgba([seed, (x * 9) as u8, (y * 13) as u8, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    Picture {
        name: name.to_owned(),
        image,
    }
}

/// What region `index` of `packed` shows, laid back over a transparent
/// picture of its source size: the loose file, if the packer is right.
fn redrawn(packed: &Packed, index: u32) -> RgbaImage {
    let frame = packed.atlas.frame(index).expect("a frame");
    let meta = packed.atlas.meta(index).expect("its meta");
    let (offset, source) = match meta.trim {
        Some(trim) => (trim.offset, trim.source),
        None => (UVec2::ZERO, frame.size()),
    };
    let mut image = RgbaImage::new(source.x, source.y);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let texel = *packed.sheet.get_pixel(frame.x + x, frame.y + y);
            image.put_pixel(offset.x + x, offset.y + y, texel);
        }
    }
    image
}

fn sample() -> Vec<Picture> {
    vec![
        picture("coin", 16, 16, 0),
        picture("hero/idle", 24, 32, 3),
        picture("hero/walk", 24, 32, 1),
        picture("wide", 40, 8, 0),
        picture("empty", 8, 8, 4),
    ]
}

#[test]
fn every_region_is_its_picture_again() {
    for trim in [false, true] {
        let options = PackOptions {
            trim,
            ..PackOptions::default()
        };
        let pictures = sample();
        let packed = pack(pictures.clone(), None, &options).expect("it fits");

        for picture in &pictures {
            let index = packed.atlas.index_of(&picture.name).expect("named");
            assert_eq!(
                redrawn(&packed, index),
                picture.image,
                "{} (trim {trim})",
                picture.name
            );
        }
    }
}

#[test]
fn a_trimmed_region_is_only_its_opaque_part() {
    let options = PackOptions {
        trim: true,
        ..PackOptions::default()
    };
    let packed = pack(sample(), None, &options).expect("it fits");

    let idle = packed.atlas.index_of("hero/idle").expect("named");
    assert_eq!(
        packed.atlas.frame(idle).map(|f| f.size()),
        Some(UVec2::new(18, 26))
    );
    assert_eq!(
        packed.atlas.meta(idle).and_then(|meta| meta.trim),
        Some(Trim {
            offset: UVec2::splat(3),
            source: UVec2::new(24, 32)
        })
    );
    let coin = packed.atlas.index_of("coin").expect("named");
    assert_eq!(
        packed.atlas.meta(coin).and_then(|meta| meta.trim),
        None,
        "nothing to trim, no trim written"
    );
}

#[test]
fn regions_are_padded_apart_and_their_edges_extruded() {
    let options = PackOptions {
        padding: 2,
        extrude: 1,
        ..PackOptions::default()
    };
    let packed = pack(sample(), None, &options).expect("it fits");

    let frames = packed.atlas.frames();
    for (i, a) in frames.iter().enumerate() {
        for b in &frames[i + 1..] {
            // An extruded texel on each side, and the padding between.
            let gap = 2 * options.extrude + options.padding;
            let apart = a.x + a.width + gap <= b.x
                || b.x + b.width + gap <= a.x
                || a.y + a.height + gap <= b.y
                || b.y + b.height + gap <= a.y;
            assert!(apart, "{a:?} and {b:?} are too close");
        }
    }
    let coin = frames[packed.atlas.index_of("coin").expect("named") as usize];
    if coin.x > 0 {
        assert_eq!(
            packed.sheet.get_pixel(coin.x - 1, coin.y),
            packed.sheet.get_pixel(coin.x, coin.y),
            "the left edge, repeated"
        );
    }
}

#[test]
fn the_sheet_is_the_smallest_power_of_two_that_fits() {
    let pictures = vec![picture("a", 30, 30, 0), picture("b", 30, 30, 0)];
    let options = PackOptions {
        padding: 0,
        extrude: 1,
        ..PackOptions::default()
    };
    let packed = pack(pictures, None, &options).expect("it fits");
    assert_eq!(packed.atlas.sheet(), UVec2::new(64, 32));
}

#[test]
fn too_much_for_the_largest_sheet_is_refused() {
    let options = PackOptions {
        max_size: 32,
        ..PackOptions::default()
    };
    assert!(matches!(
        pack(vec![picture("big", 40, 4, 0)], None, &options),
        Err(PackError::TooBig { max_size: 32 })
    ));
}

#[test]
fn a_repack_keeps_every_index_and_pivot_by_name() {
    let options = PackOptions::default();
    let mut first = pack(sample(), None, &options).expect("it fits").atlas;
    let wide = first.index_of("wide").expect("named");
    // A pivot set in the atlas editor since.
    let mut edited = Atlas::new(first.sheet(), Vec::new());
    for i in 0..first.len() as u32 {
        let mut meta = first.meta(i).expect("meta").clone();
        if i == wide {
            meta.pivot = Some(Vec2::new(0.25, 1.0));
        }
        edited.push(first.frame(i).expect("frame"), meta);
    }
    first = edited;

    let mut pictures = sample();
    pictures.retain(|p| p.name != "hero/idle");
    pictures.push(picture("aaa-new", 4, 4, 0));
    let second = pack(pictures, Some(&first), &options).expect("it fits");

    for name in ["coin", "hero/walk", "wide", "empty"] {
        assert_eq!(second.atlas.index_of(name), first.index_of(name), "{name}");
    }
    let idle = first.index_of("hero/idle").expect("was there") as usize;
    assert!(
        second.atlas.frames()[idle].is_empty(),
        "a gone file keeps its slot, empty"
    );
    assert_eq!(second.gone, vec!["hero/idle".to_owned()]);
    assert_eq!(second.atlas.index_of("aaa-new"), Some(first.len() as u32));
    assert_eq!(
        second.atlas.meta(wide).and_then(|meta| meta.pivot),
        Some(Vec2::new(0.25, 1.0))
    );
}

#[test]
fn two_pictures_of_one_name_are_refused() {
    assert!(matches!(
        pack(
            vec![picture("a", 2, 2, 0), picture("a", 2, 2, 0)],
            None,
            &PackOptions::default()
        ),
        Err(PackError::Duplicate(_))
    ));
}

fn write(path: &Path, picture: &Picture) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
    picture.image.save(path).expect("the PNG writes");
}

#[test]
fn a_folder_packs_beside_itself_and_loads_as_an_atlas() {
    let root = scratch_root();
    let folder = root.join("sprites/pieces");
    for picture in sample() {
        write(&folder.join(format!("{}.png", picture.name)), &picture);
    }
    let atlas = default_atlas_path(&folder);
    assert_eq!(atlas, root.join("sprites/pieces.atlas.ron"));

    let report = pack_folder(&folder, &atlas, Some(&root), &PackOptions::default())
        .expect("the pack writes");

    assert_eq!(report.sheet, root.join("sprites/pieces.png"));
    assert_eq!(report.regions, 5);
    assert_eq!(
        report.texture.as_ref().map(AssetPath::as_str),
        Some("sprites/pieces.png")
    );
    let mut atlases = crate::Atlases::new(&root);
    let handle = atlases.load(&AssetPath::new("sprites/pieces.atlas.ron").expect("valid"));
    let loaded = atlases.get(handle);
    assert_eq!(loaded.len(), 5);
    assert_eq!(loaded.name(0), Some("coin"));
    assert_eq!(loaded.sheet(), report.size);
    let sheet = image::open(&report.sheet)
        .expect("the sheet reads")
        .to_rgba8();
    assert_eq!(UVec2::from(sheet.dimensions()), report.size);

    // Packed again with the sheet inside the folder: it is not a picture.
    let inside = folder.join("all.atlas.ron");
    pack_folder(&folder, &inside, None, &PackOptions::default()).expect("first");
    let again = pack_folder(&folder, &inside, None, &PackOptions::default()).expect("second");
    assert_eq!(again.regions, 5, "its own sheet was not packed into it");
    assert_eq!(again.texture, None, "no root, no hint");
}

#[test]
fn an_atlas_packed_into_a_new_folder_still_names_its_sheet() {
    let root = scratch_root();
    let folder = root.join("loose");
    write(&folder.join("a.png"), &picture("a", 4, 4, 0));
    let atlas = root.join("packed/new/loose.atlas.ron");

    let report = pack_folder(&folder, &atlas, Some(&root), &PackOptions::default())
        .expect("the pack writes");

    assert_eq!(
        report.texture.as_ref().map(AssetPath::as_str),
        Some("packed/new/loose.png")
    );
}

#[test]
fn an_unreadable_previous_atlas_is_left_alone() {
    let root = scratch_root();
    let folder = root.join("pieces");
    write(&folder.join("a.png"), &picture("a", 4, 4, 0));
    let atlas = root.join("pieces.atlas.ron");
    std::fs::write(&atlas, "(version: 2, frames: [").expect("writes");

    let result = pack_folder(&folder, &atlas, None, &PackOptions::default());

    assert!(matches!(result, Err(PackError::Previous(..))));
    assert_eq!(
        std::fs::read_to_string(&atlas).expect("still there"),
        "(version: 2, frames: ["
    );
}

#[test]
fn the_file_written_reads_back_as_the_same_atlas() {
    let packed = pack(
        sample(),
        None,
        &PackOptions {
            trim: true,
            ..PackOptions::default()
        },
    )
    .expect("it fits");
    let text = AtlasFile::from_atlas(&packed.atlas)
        .to_ron()
        .expect("writes");
    let back = ron::from_str::<AtlasFile>(&text)
        .expect("reads")
        .into_atlas()
        .expect("a slicing");
    assert_eq!(back, packed.atlas, "{text}");
}
