//! The hierarchy's Create menu: 3D entities — an empty, a camera, the
//! built-in meshes.
//!
//! Godot's "Add Node" offers `Node3D` and Unity's hierarchy "Create Empty";
//! this is that, filed under the dimension it is placed in, since a 3D
//! entity is made here and nowhere else until the 3D view (stage 145).

use voltra_assets::BuiltinMesh;
use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::{Quat, Vec3};
use voltra_scene::{
    Camera3D, DirectionalLight3D, Environment3D, MeshInstance3D, Name, PointLight3D, SceneId,
    SpotLight3D, Transform3D,
};

use crate::editor::Editor;
use crate::spawn;

/// The menu, and the spawn it was asked for as one undo entry.
pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    ui.menu_button("Create", |ui| {
        ui.menu_button("3D", |ui| {
            let empty = ui
                .button("Empty")
                .on_hover_text("an entity with a Transform3D at the origin, to parent 3D under");
            if empty.clicked() {
                spawn::record(editor, frame, "Create 3D empty", |frame| {
                    empty_3d(frame.world)
                });
                ui.close();
            }
            let camera = ui
                .button("Camera")
                .on_hover_text("a Camera3D 5 m back and 2 m up, looking at the origin");
            if camera.clicked() {
                spawn::record(editor, frame, "Create 3D camera", |frame| {
                    camera_3d(frame.world)
                });
                ui.close();
            }
            let light = ui
                .button("Directional Light")
                .on_hover_text("a DirectionalLight3D shining down from the upper right, ahead");
            if light.clicked() {
                spawn::record(editor, frame, "Create 3D light", |frame| {
                    light_3d(frame.world)
                });
                ui.close();
            }
            let point = ui
                .button("Point Light")
                .on_hover_text("a PointLight3D 2 m up, its reach 5 m");
            if point.clicked() {
                spawn::record(editor, frame, "Create point light", |frame| {
                    point_light_3d(frame.world)
                });
                ui.close();
            }
            let spot = ui
                .button("Spot Light")
                .on_hover_text("a SpotLight3D 3 m up, shining straight down");
            if spot.clicked() {
                spawn::record(editor, frame, "Create spot light", |frame| {
                    spot_light_3d(frame.world)
                });
                ui.close();
            }
            let environment = ui
                .button("Environment")
                .on_hover_text("the world's Environment3D: its ambient, background and fog");
            if environment.clicked() {
                spawn::record(editor, frame, "Create environment", |frame| {
                    environment_3d(frame.world)
                });
                ui.close();
            }
            ui.separator();
            for mesh in BuiltinMesh::ALL {
                let button = ui
                    .button(title(mesh))
                    .on_hover_text(format!("a MeshInstance3D of builtin:{}", mesh.name()));
                if button.clicked() {
                    spawn::record(editor, frame, "Create 3D mesh", |frame| {
                        mesh_3d(frame.world, mesh)
                    });
                    ui.close();
                }
            }
        });
    });
}

/// A named entity with an identity and a [`Transform3D`] at the origin.
fn empty_3d(world: &mut voltra_ecs::World) -> Entity {
    named_3d(world, "Empty3D", Transform3D::default())
}

/// A camera framing the origin from where a person would stand.
fn camera_3d(world: &mut voltra_ecs::World) -> Entity {
    let at =
        Transform3D::from_translation(Vec3::new(0.0, 2.0, 5.0)).looking_at(Vec3::ZERO, Vec3::Y);
    let entity = named_3d(world, Camera3D::NAME, at);
    world.insert(entity, Camera3D::default());
    entity
}

/// A sun 50° above the horizon, from over the right shoulder of a
/// camera made here, 4 m up so it is found by the gizmo.
fn light_3d(world: &mut voltra_ecs::World) -> Entity {
    let at = Transform3D::from_translation(Vec3::new(0.0, 4.0, 0.0)).with_rotation(
        Quat::from_rotation_y(35f32.to_radians()) * Quat::from_rotation_x(-50f32.to_radians()),
    );
    let entity = named_3d(world, DirectionalLight3D::NAME, at);
    world.insert(entity, DirectionalLight3D::default());
    entity
}

/// A point light over the origin, at head height.
fn point_light_3d(world: &mut voltra_ecs::World) -> Entity {
    let at = Transform3D::from_translation(Vec3::new(0.0, 2.0, 0.0));
    let entity = named_3d(world, PointLight3D::NAME, at);
    world.insert(entity, PointLight3D::default());
    entity
}

/// A spot light over the origin shining straight down: its −Z turned to
/// −Y.
fn spot_light_3d(world: &mut voltra_ecs::World) -> Entity {
    let at = Transform3D::from_translation(Vec3::new(0.0, 3.0, 0.0))
        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    let entity = named_3d(world, SpotLight3D::NAME, at);
    world.insert(entity, SpotLight3D::default());
    entity
}

/// The world's environment, the default ambient; where it stands does not
/// matter.
fn environment_3d(world: &mut voltra_ecs::World) -> Entity {
    let entity = named_3d(world, Environment3D::NAME, Transform3D::default());
    world.insert(entity, Environment3D::default());
    entity
}

/// A built-in mesh standing on the ground at the origin.
fn mesh_3d(world: &mut voltra_ecs::World, mesh: BuiltinMesh) -> Entity {
    let bottom = mesh.generate().bounds().map_or(0.0, |b| b.min.y);
    let at = Transform3D::from_translation(Vec3::new(0.0, -bottom, 0.0));
    let entity = named_3d(world, title(mesh), at);
    world.insert(entity, MeshInstance3D::new(mesh));
    entity
}

fn named_3d(world: &mut voltra_ecs::World, name: &str, at: Transform3D) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, at);
    entity
}

/// A built-in mesh's name as a menu and an entity show it.
fn title(mesh: BuiltinMesh) -> &'static str {
    match mesh {
        BuiltinMesh::Cube => "Cube",
        BuiltinMesh::Sphere => "Sphere",
        BuiltinMesh::Plane => "Plane",
        BuiltinMesh::Capsule => "Capsule",
        BuiltinMesh::Cylinder => "Cylinder",
    }
}

#[cfg(test)]
mod tests {
    use voltra_ecs::World;
    use voltra_scene::hierarchy;

    use super::*;

    #[test]
    fn an_empty_is_3d_at_the_origin_and_takes_3d_children_only() {
        let mut world = World::new();
        let empty = empty_3d(&mut world);
        assert_eq!(
            world.get::<Transform3D>(empty),
            Some(&Transform3D::default())
        );
        assert!(world.get::<SceneId>(empty).is_some());

        let child = empty_3d(&mut world);
        hierarchy::set_parent(&mut world, child, empty).expect("3D under 3D");
        let flat = world.spawn();
        world.insert(flat, SceneId::new());
        world.insert(flat, voltra_scene::Transform2D::default());
        assert_eq!(
            hierarchy::set_parent(&mut world, flat, empty),
            Err(hierarchy::HierarchyError::MixedDimensions)
        );
    }

    #[test]
    fn a_created_light_shines_down_and_away_onto_what_a_created_camera_sees() {
        let mut world = World::new();
        let light = light_3d(&mut world);
        environment_3d(&mut world);
        let camera = camera_3d(&mut world);
        let view = voltra_scene::camera3d::view(&world, camera, 1.0).expect("a camera");
        let gathered = voltra_scene::lighting3d::gather(&world, camera, &view);
        let sun = gathered.lighting.directional[0];
        assert_eq!(gathered.directional, [light]);
        // Down, and away from a camera on +Z: what it sees is lit.
        assert!(
            sun.direction.y < -0.7 && sun.direction.z < 0.0,
            "{}",
            sun.direction
        );
        assert_eq!(
            gathered.lighting.ambient,
            [Environment3D::DEFAULT_AMBIENT; 3]
        );
    }

    #[test]
    fn created_point_and_spot_lights_reach_what_a_created_camera_sees() {
        let mut world = World::new();
        let point = point_light_3d(&mut world);
        let spot = spot_light_3d(&mut world);
        let camera = camera_3d(&mut world);
        let view = voltra_scene::camera3d::view(&world, camera, 1.0).expect("a camera");
        let gathered = voltra_scene::lighting3d::gather(&world, camera, &view);
        let reached: Vec<u32> = gathered.punctual.iter().map(|r| r.entity).collect();
        assert_eq!(reached, [point.index(), spot.index()]);
        let cone = gathered.lighting.punctual[1].spot.expect("a spot");
        assert!(
            cone.direction.abs_diff_eq(Vec3::NEG_Y, 1e-6),
            "{}",
            cone.direction
        );
        // A mesh made at the origin is lit by both.
        let mesh = mesh_3d(&mut world, BuiltinMesh::Cube);
        let bounds = voltra_render::render3d::Aabb3::new(Vec3::splat(-0.5), Vec3::splat(0.5));
        let layers = world
            .get::<MeshInstance3D>(mesh)
            .expect("a mesh")
            .light_layers;
        let picked = voltra_scene::lighting3d::pick(
            &gathered.punctual,
            &gathered.directional_layers,
            &bounds,
            layers,
        );
        assert_eq!(picked.reached, 2);
    }

    #[test]
    fn a_created_camera_frames_the_origin_and_a_mesh_stands_on_the_ground() {
        let mut world = World::new();
        let camera = camera_3d(&mut world);
        let view = voltra_scene::camera3d::view(&world, camera, 1.0).expect("a camera");
        let toward = (Vec3::ZERO - view.eye).normalize();
        assert!(view.forward.dot(toward) > 0.9999, "{}", view.forward);
        for mesh in BuiltinMesh::ALL {
            let e = mesh_3d(&mut world, mesh);
            let y = world.get::<Transform3D>(e).expect("placed").translation.y;
            let bottom = mesh.generate().bounds().expect("vertices").min.y;
            assert!(
                (y + bottom).abs() < 1e-6,
                "{mesh:?} stands at {}",
                y + bottom
            );
            assert_eq!(
                world.get::<MeshInstance3D>(e).map(|m| &m.mesh),
                Some(&voltra_assets::MeshPath::Builtin(mesh))
            );
        }
    }
}
