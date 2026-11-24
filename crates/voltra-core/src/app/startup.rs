//! The second half of startup: everything that needs the renderer.
//!
//! Natively it runs inside `resumed`, right after the device; on the web
//! from `user_event`, once the awaited device arrives (`app/web.rs`).

use voltra_assets::{AssetWatcher, Atlases, Clips, Fonts, Textures};
use voltra_render::{Filter, RenderTarget, Renderer};

use super::{gamepads, ui_frame, App};
use crate::time::Clock;
use crate::ui::EguiLayer;

impl App {
    /// The rest of `resumed`, once there is a renderer: the stores, the
    /// world's handles resolved, the UI layer, and the first frame asked for.
    pub(super) fn finish_resume(&mut self, mut renderer: Renderer) {
        let Some(window) = self.window.clone() else {
            log::error!("a renderer arrived with no window to draw in");
            return;
        };
        let size = window.inner_size();
        // The canvas may have been laid out while the device was on its way,
        // its `Resized` arriving when there was nothing to resize. Natively a
        // no-op: the size is the one the renderer was just built at.
        renderer.resize(size.width, size.height);

        let asset_root = self
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        log::info!("asset root: {}", asset_root.display());

        // The same layout object the sprite pipeline was built with —
        // `Renderer` owns both, so `texture_layout()` is guaranteed to be it.
        let mut textures = Textures::new(
            renderer.context().device(),
            renderer.context().queue(),
            renderer.texture_layout(),
            asset_root.clone(),
        );
        textures.set_filter(self.texture_filter);
        textures.set_target_format(renderer.context().format());

        // `App::world` is public and documented to be populated before `run`,
        // so by the time there is a device to upload with, the world may
        // already hold sprites naming a texture they have never resolved.
        // Without this they draw untextured forever: the only other callers of
        // `set_texture` are the inspector and Open, and a game has neither.
        // Instances first, so what they are brought in step with is resolved
        // with the rest.
        self.sync_all_instances();
        ui_frame::resolve_world_textures(
            &mut self.world,
            &mut textures,
            renderer.context().device(),
            renderer.context().queue(),
        );

        // The slicing half of the same problem, and for the same reason: a
        // sprite naming an atlas has never met an `Atlases` either.
        let mut atlases = Atlases::new(asset_root.clone());
        ui_frame::resolve_world_atlases(&mut self.world, &mut atlases);
        self.atlases = Some(atlases);
        self.silhouettes = Some(voltra_assets::Silhouettes::new(asset_root.clone()));

        self.load_input_map(&asset_root);
        self.load_sorting_layers(&asset_root);
        self.gamepads = gamepads::Gamepads::connect();

        // And the legible part: a text in a world populated before `run`
        // has never met a `Fonts`.
        let mut fonts = Fonts::new(asset_root.clone());
        voltra_scene::text::resolve_fonts(&mut self.world, &mut fonts);
        self.fonts = Some(fonts);
        self.open_locale(&asset_root);
        self.materials = Some(voltra_assets::Materials::new(asset_root.clone()));

        // The audible third of the same problem: a source in a world populated
        // before `run` has never met a `Clips` either.
        let mut clips = Clips::new(asset_root.clone());
        ui_frame::resolve_world_clips(&mut self.world, &mut clips);
        self.clips = Some(clips);

        // Last of the three, and the only one that can fail without stopping
        // anything: with no device this is the silent `Audio` and the game
        // runs on.
        self.audio.connect();

        if self.hot_reload {
            match AssetWatcher::new(&asset_root) {
                Ok(watcher) => self.watcher = Some(watcher),
                // Not fatal. Refusing to open the editor because a watch handle
                // could not be had would trade a working session for a feature
                // nobody has used yet this run.
                Err(e) => log::error!("hot reload disabled: {e}"),
            }
        }

        if self.ui.is_some() {
            let device = renderer.context().device().clone();
            let format = renderer.context().format();

            // Same format as the window so the UI compositing step has nothing
            // to convert, and full window size until a panel says otherwise.
            let target = RenderTarget::new(
                &device,
                "scene-viewport",
                format,
                size.width,
                size.height,
                Filter::Linear,
            );
            let post = RenderTarget::new(
                &device,
                "scene-viewport-graded",
                format,
                size.width,
                size.height,
                Filter::Linear,
            );
            let mut egui = EguiLayer::new(&window, &device, format);
            self.viewport = Some(egui.register_view(&device, post.raw_view(), Filter::Linear));
            self.requested_size = (target.width(), target.height());
            self.scene_target = Some(target);
            self.post_target = Some(post);
            self.egui = Some(egui);
        }

        self.renderer = Some(renderer);
        self.textures = Some(textures);
        self.refresh_game_window();

        // Device creation takes long enough to produce a huge first delta;
        // restarting the clock here keeps frame one honest.
        self.clock = Clock::new();
        // The first redraw may have been asked for before there was anything
        // to draw it with.
        window.request_redraw();
        #[cfg(target_arch = "wasm32")]
        super::web::started(&window);
    }
}

impl App {
    /// Opens the string tables under `root` in the language the game was
    /// built to start in.
    pub(super) fn open_locale(&mut self, root: &std::path::Path) {
        let language = if self.language.is_empty() {
            "en"
        } else {
            self.language.as_str()
        };
        self.locale = voltra_scene::Locale::open(root, language);
        self.locale.localize(&mut self.world);
    }
}
