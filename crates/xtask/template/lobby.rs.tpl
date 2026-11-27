//! The lobby screen this game was made with (`assets/prefabs/lobby.prefab.ron`,
//! filled by `voltra_core::lobby::LobbyScreen`) opened headless: Host on
//! this machine's loopback, Ready, Start — the match begins. A game opens
//! it from its menu, and loads its level when it answers `Started`.

use std::cell::RefCell;
use std::rc::Rc;

use voltra_core::lobby::{LobbyOutcome, LobbyScreen, LobbyScreenConfig};
use voltra_core::net::{Advertise, HostOptions, LobbyConfig};
use voltra_core::{App, Headless, KeyCode};
use voltra_render::glam::Vec2;
use voltra_scene::Name;

const FRAME: f32 = 1.0 / 60.0;

#[test]
fn the_lobby_screen_hosts_and_starts_a_match() {
    let config = LobbyScreenConfig {
        host: HostOptions {
            bind: [127, 0, 0, 1].into(),
            udp_port: Some(0),
            websocket_port: None,
            discovery: Some(Advertise {
                port: 0,
                ..Advertise::default()
            }),
            ..HostOptions::default()
        },
        // A match of one, so a test alone can start it.
        lobby: LobbyConfig {
            min_players: 1,
            ..LobbyConfig::default()
        },
        ..LobbyScreenConfig::for_game("{{package}}")
    };
    let outcomes: Rc<RefCell<Vec<LobbyOutcome>>> = Rc::default();
    let seen = outcomes.clone();
    let mut screen: Option<LobbyScreen> = None;
    let app = App::default().with_update(move |tick| match screen.as_mut() {
        None => screen = Some(LobbyScreen::open(tick, &config).expect("the lobby screen opens")),
        Some(open) => seen.borrow_mut().push(open.update(tick)),
    });
    let mut session = Headless::new(app).with_screen(Vec2::new(1280.0, 720.0));
    session.step(2, FRAME);

    for button in ["Host", "Ready", "Start"] {
        let control = session
            .world()
            .query::<Name>()
            .find(|(_, name)| name.as_str() == button)
            .map(|(entity, _)| entity)
            .unwrap_or_else(|| panic!("the lobby screen has {button}"));
        session.focus(Some(control));
        session.input_mut().on_key(KeyCode::Enter, true, false);
        session.step(1, FRAME);
        session.input_mut().on_key(KeyCode::Enter, false, false);
        session.step(3, FRAME);
    }
    assert!(session.net().is_server(), "hosting");
    assert!(
        outcomes.borrow().contains(&LobbyOutcome::Started),
        "the match started"
    );
}
