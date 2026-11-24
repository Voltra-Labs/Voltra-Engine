//! The Network window: how many clients Play starts, how bad the network
//! between them is, and what each one sees.
//!
//! Unity's Multiplayer Play Mode and its Network Simulator in one window:
//! the conditions apply to every link of the session live (see
//! [`crate::netplay`]); the counters are the connections' own, and the same
//! numbers reach the Profiler as `net …` counters every frame.

use voltra_core::egui::{self, Color32, Sense, Ui, Vec2};
use voltra_core::net::PeerId;
use voltra_core::{Headless, UiFrame};
use voltra_scene::{Replicated, Transform};

use crate::editor::Editor;
use crate::netplay::MAX_CLIENTS;

/// A client's map of its replicated entities, in points.
const MAP: f32 = 120.0;

pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    if !editor.netplay.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Network")
        .open(&mut open)
        .default_size([420.0, 480.0])
        .show(ui.ctx(), |ui| contents(editor, ui, frame));
    editor.netplay.open = open;
}

fn contents(editor: &mut Editor, ui: &mut Ui, frame: &UiFrame<'_>) {
    let running = editor.netplay.is_running();
    ui.add_enabled_ui(!running, |ui| {
        ui.horizontal(|ui| {
            ui.label("Clients on Play");
            ui.add(egui::DragValue::new(&mut editor.netplay.clients).range(0..=MAX_CLIENTS));
        })
        .response
        .on_hover_text(
            "Play hosts this world as a listen server and starts this many clients \
             in the editor. 0 plays offline.",
        );
    });

    ui.separator();
    ui.strong("Conditions, every link, both ways");
    let c = &mut editor.netplay.conditions;
    let mut latency = c.latency.as_secs_f64() * 1000.0;
    let mut jitter = c.jitter.as_secs_f64() * 1000.0;
    let mut loss = f64::from(c.loss) * 100.0;
    let mut bandwidth = f64::from(c.bandwidth) / 1000.0;
    ui.add(
        egui::Slider::new(&mut latency, 0.0..=500.0)
            .text("latency")
            .suffix(" ms"),
    );
    ui.add(
        egui::Slider::new(&mut jitter, 0.0..=200.0)
            .text("jitter")
            .suffix(" ms"),
    );
    ui.add(
        egui::Slider::new(&mut loss, 0.0..=50.0)
            .text("loss")
            .suffix(" %"),
    );
    ui.add(
        egui::Slider::new(&mut bandwidth, 0.0..=1000.0)
            .text("bandwidth (0: none)")
            .suffix(" KB/s"),
    );
    c.latency = std::time::Duration::from_secs_f64(latency.max(0.0) / 1000.0);
    c.jitter = std::time::Duration::from_secs_f64(jitter.max(0.0) / 1000.0);
    c.loss = (loss / 100.0).clamp(0.0, 1.0) as f32;
    c.bandwidth = (bandwidth.max(0.0) * 1000.0) as u32;

    ui.separator();
    if !running {
        ui.weak("not playing with clients");
        return;
    }
    ui.strong("Server");
    egui::Grid::new("network-server")
        .striped(true)
        .show(ui, |ui| {
            header(ui);
            for peer in frame.net.peers().collect::<Vec<_>>() {
                connection(ui, &format!("{peer}"), frame.net, peer);
            }
        });
    ui.separator();
    for (n, client) in editor.netplay.clients().iter().enumerate() {
        ui.strong(format!("Client {}", n + 1));
        egui::Grid::new(("network-client", n))
            .striped(true)
            .show(ui, |ui| {
                header(ui);
                connection(ui, "server", client.net(), PeerId::SERVER);
            });
        if let Some(stats) = client
            .net()
            .replication()
            .and_then(|r| r.client())
            .map(|c| c.stats())
        {
            ui.label(format!(
                "{} entities, drawn {:.0} ms behind (jitter {:.0} ms)",
                stats.entities,
                stats.delay.as_secs_f64() * 1000.0,
                stats.jitter.as_secs_f64() * 1000.0
            ));
        }
        map(ui, client, frame);
    }
}

fn header(ui: &mut Ui) {
    for title in ["peer", "RTT", "sent", "received", "packets out/in"] {
        ui.weak(title);
    }
    ui.end_row();
}

fn connection(ui: &mut Ui, name: &str, net: &voltra_core::Net, peer: PeerId) {
    ui.label(name);
    match net.stats(peer) {
        Some(stats) => {
            ui.label(format!("{:.0} ms", stats.rtt.as_secs_f64() * 1000.0));
            ui.label(kilobytes(stats.bytes_sent));
            ui.label(kilobytes(stats.bytes_received));
            ui.label(format!("{}/{}", stats.packets_sent, stats.packets_received));
        }
        None => {
            ui.weak("connecting…");
        }
    }
    ui.end_row();
}

fn kilobytes(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1000.0)
}

/// The client's replicated entities as dots, framed on the server's: a
/// lagging or missing one shows as a dot apart from the rest.
fn map(ui: &mut Ui, client: &Headless, frame: &UiFrame<'_>) {
    let positions = |world: &voltra_ecs::World| -> Vec<voltra_render::glam::Vec2> {
        world
            .query::<Replicated>()
            .filter_map(|(entity, _)| world.get::<Transform>(entity))
            .map(|t| t.translation)
            .collect()
    };
    let server = positions(frame.world);
    let seen = positions(client.world());
    let (min, max) = server.iter().chain(&seen).fold(
        (
            voltra_render::glam::Vec2::splat(f32::MAX),
            voltra_render::glam::Vec2::splat(f32::MIN),
        ),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(MAP), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    if seen.is_empty() && server.is_empty() {
        return;
    }
    let span = (max - min).max(voltra_render::glam::Vec2::splat(1.0));
    let to_screen = |p: voltra_render::glam::Vec2| {
        let t = (p - min) / span;
        egui::pos2(
            rect.left() + 6.0 + t.x * (MAP - 12.0),
            rect.bottom() - 6.0 - t.y * (MAP - 12.0),
        )
    };
    for p in &server {
        painter.circle_filled(to_screen(*p), 4.0, Color32::from_gray(90));
    }
    for p in &seen {
        painter.circle_filled(to_screen(*p), 2.5, ui.visuals().selection.bg_fill);
    }
}
