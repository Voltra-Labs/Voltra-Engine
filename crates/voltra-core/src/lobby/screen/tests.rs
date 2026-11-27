//! The shipped lobby prefab opened headless and worked as a player would:
//! hosting and starting alone; finding a host on the LAN — probed over
//! loopback, which every container passes — and joining it from its row.

use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr};
use std::rc::Rc;
use std::time::Duration;

use voltra_ecs::{Entity, World};
use voltra_net::{Advertise, HostOptions, LobbyConfig, SearchOptions};
use voltra_render::glam::Vec2;
use voltra_scene::{Name, Text, UiButton, UiTextField};

use super::*;
use crate::{App, Headless, KeyCode};

const FRAME: f32 = 1.0 / 60.0;

/// What the screen answered, frame by frame, and the screen itself.
#[derive(Default)]
struct Shared {
    screen: Option<LobbyScreen>,
    outcomes: Vec<LobbyOutcome>,
}

struct Harness {
    session: Headless,
    shared: Rc<RefCell<Shared>>,
}

fn loopback() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// A screen on loopback ports any test may take: hosting on free ports,
/// answering probes on a free one.
fn config(name: &str, min_players: usize) -> LobbyScreenConfig {
    LobbyScreenConfig {
        game: "lobby-test".into(),
        host: HostOptions {
            bind: loopback(),
            udp_port: Some(0),
            websocket_port: None,
            discovery: Some(Advertise {
                port: 0,
                name: String::new(),
                ..Advertise::default()
            }),
            ..HostOptions::default()
        },
        lobby: LobbyConfig {
            name: name.into(),
            min_players,
            ..LobbyConfig::default()
        },
        ..LobbyScreenConfig::default()
    }
}

/// A game that opens the screen on its first frame and runs it after.
fn harness(config: LobbyScreenConfig) -> Harness {
    let shared: Rc<RefCell<Shared>> = Rc::default();
    let held = shared.clone();
    let mut opened = false;
    let app = App::default().with_update(move |tick| {
        let mut shared = held.borrow_mut();
        if !opened {
            opened = true;
            match LobbyScreen::open(tick, &config) {
                Ok(screen) => shared.screen = Some(screen),
                Err(e) => panic!("the lobby screen did not open: {e}"),
            }
            return;
        }
        if let Some(screen) = shared.screen.as_mut() {
            let outcome = screen.update(tick);
            shared.outcomes.push(outcome);
        }
    });
    let mut session = Headless::new(app).with_screen(Vec2::new(1280.0, 720.0));
    session.step(2, FRAME);
    Harness { session, shared }
}

fn named(world: &World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("nothing named {name:?}"))
}

fn is_hidden(world: &World, name: &str) -> bool {
    voltra_scene::ui::hidden::is_hidden(world, named(world, name))
}

/// The texts of `control` and under it, in order, joined by ` | `: a
/// button's one label, a row's name and detail.
fn label(world: &World, control: Entity) -> String {
    std::iter::once(control)
        .chain(voltra_scene::hierarchy::descendants(world, control))
        .filter_map(|e| world.get::<Text>(e).map(|t| t.content.clone()))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn disabled(world: &World, name: &str) -> bool {
    world
        .get::<UiButton>(named(world, name))
        .is_some_and(|b| b.disabled)
}

/// Focuses `control` and presses accept on it.
fn press_entity(h: &mut Harness, control: Entity) {
    h.session.focus(Some(control));
    h.session.input_mut().on_key(KeyCode::Enter, true, false);
    h.session.step(1, FRAME);
    h.session.input_mut().on_key(KeyCode::Enter, false, false);
    h.session.step(1, FRAME);
}

fn press(h: &mut Harness, name: &str) {
    let control = named(h.session.world(), name);
    press_entity(h, control);
}

fn status(h: &Harness) -> String {
    label(h.session.world(), named(h.session.world(), "Status"))
}

fn player_labels(h: &Harness) -> Vec<String> {
    let shared = h.shared.borrow();
    let screen = shared.screen.as_ref().expect("open");
    screen
        .player_rows()
        .iter()
        .map(|&row| label(h.session.world(), row))
        .collect()
}

#[test]
fn the_screen_opens_on_its_menu_with_the_names_filled_in() {
    let h = harness(config("Ada", 2));
    let world = h.session.world();

    assert!(!is_hidden(world, "Lobby menu"));
    assert!(is_hidden(world, "Lobby room"), "no lobby yet");
    assert!(
        is_hidden(world, "Server row"),
        "the templates are kept, hidden"
    );
    assert!(is_hidden(world, "Player row"));
    let field = |name| {
        world
            .get::<UiTextField>(named(world, name))
            .map(|f| f.text.clone())
            .expect("a field")
    };
    assert_eq!(field("Name"), "Ada");
    assert_eq!(field("Address"), "udp://127.0.0.1:7777");
    assert!(
        h.session.net().has_lobby(),
        "the screen turned the lobby on"
    );
}

#[test]
fn hosting_alone_shows_the_room_and_the_host_starts_once_ready() {
    let mut h = harness(config("Ada", 1));
    press(&mut h, "Host");
    h.session.step(2, FRAME);

    let world = h.session.world();
    assert!(h.session.net().is_server(), "{}", status(&h));
    assert!(is_hidden(world, "Lobby menu"));
    assert!(!is_hidden(world, "Lobby room"));
    assert_eq!(player_labels(&h), ["Ada | you · host · not ready"]);
    assert!(disabled(world, "Start"), "nobody is ready");

    press(&mut h, "Ready");
    h.session.step(2, FRAME);
    assert_eq!(player_labels(&h), ["Ada | you · host · ready"]);
    assert!(!disabled(h.session.world(), "Start"));

    press(&mut h, "Start");
    h.session.step(2, FRAME);
    assert!(
        h.shared.borrow().outcomes.contains(&LobbyOutcome::Started),
        "{}",
        status(&h)
    );

    // Leave ends the session and goes back to the menu.
    press(&mut h, "Leave");
    h.session.step(3, FRAME);
    assert!(!h.session.net().is_active());
    assert!(!is_hidden(h.session.world(), "Lobby menu"));
}

#[test]
fn a_host_found_on_the_lan_is_joined_from_its_row() {
    let mut host = harness(config("Ada", 2));
    press(&mut host, "Host");
    host.session.step(2, FRAME);
    let probe_at = host
        .session
        .net()
        .discovery_addr()
        .expect("answering probes");

    let mut guest_config = config("Bob", 2);
    guest_config.search = SearchOptions {
        broadcast: false,
        targets: vec![probe_at],
        timeout: Duration::from_secs(3),
        ..SearchOptions::default()
    };
    let mut guest = harness(guest_config);
    press(&mut guest, "Find on LAN");
    let rows = |g: &Harness| {
        g.shared
            .borrow()
            .screen
            .as_ref()
            .map(|s| s.server_rows().to_vec())
            .unwrap_or_default()
    };
    for _ in 0..300 {
        host.session.step(1, FRAME);
        guest.session.step(1, FRAME);
        if !rows(&guest).is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let row = *rows(&guest).first().expect("the host listed");
    assert_eq!(label(guest.session.world(), row), "Ada's game | 1/8");

    press_entity(&mut guest, row);
    for _ in 0..300 {
        host.session.step(1, FRAME);
        guest.session.step(1, FRAME);
        if player_labels(&guest).len() == 2 && player_labels(&host).len() == 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        player_labels(&guest),
        ["Ada | host · not ready", "Bob | you · not ready"],
        "{}",
        status(&guest)
    );
    assert!(disabled(guest.session.world(), "Start"), "not the host");
}

#[test]
fn someone_elses_words_are_never_read_as_markup() {
    // The shipped prefab draws them plain.
    let mut h = harness(config("Ada", 1));
    h.session.step(1, FRAME);
    let world = h.session.world();
    for part in [
        "Status",
        "Server label",
        "Server detail",
        "Player label",
        "Player detail",
    ] {
        let text = world.get::<Text>(named(world, part)).expect("a text");
        assert!(!text.rich, "{part} is rich");
    }

    // A game's own prefab may make them rich: the words are escaped there,
    // and come out as typed.
    let hostile = "[size=1e30][color=#0000][font=../x.ttf]Eve[icon=a.png]";
    let mut world = World::new();
    let rich = world.spawn();
    world.insert(rich, Text::new("before"));
    let plain = world.spawn();
    world.insert(
        plain,
        Text {
            rich: false,
            ..Text::new("before")
        },
    );
    super::list::set_words(&mut world, Some(rich), hostile);
    super::list::set_words(&mut world, Some(plain), hostile);
    let shown = world.get::<Text>(rich).expect("a text");
    let parsed = voltra_scene::text::markup::parse(&shown.content);
    assert_eq!(parsed.text, hostile, "drawn as typed");
    assert_eq!(parsed.runs.len(), 1);
    assert_eq!(
        world.get::<Text>(plain).map(|t| t.content.as_str()),
        Some(hostile)
    );
}
