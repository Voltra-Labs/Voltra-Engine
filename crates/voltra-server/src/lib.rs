//! A dedicated server: a scene run headless and authoritatively at a fixed
//! tick, with clients joining over UDP and WebSockets.
//!
//! The binary (`voltra-server`) runs a scene with no game code; a game
//! that wants its own logic on the server builds its own binary on
//! [`Server`] the way it builds its player — `Server::open(game_app, …)` —
//! and its ticks run there exactly as in the player, `tick.net` included.
//! Unreal's dedicated server and Godot's `--headless` export are the same
//! idea: the game, minus the window, plus the sockets.
//!
//! A game that hosts *and plays* — a listen server — needs none of this:
//! it calls `tick.net.host(&HostOptions { listen: true, .. })` from its own
//! player.

#![warn(missing_docs)]

use std::fmt;
use std::time::{Duration, Instant};

use voltra_assets::AssetPath;
use voltra_core::net::{Address, HostOptions, NetEvent, PeerId, SessionError};
use voltra_core::{App, Headless};
use voltra_scene::SceneError;

pub mod args;
pub mod cli;
pub mod pacer;

pub use pacer::Pacer;

/// The tick rate a server runs at unless told otherwise: the physics step's.
pub const DEFAULT_TICK_RATE: u32 = 60;

/// How a server runs.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerOptions {
    /// Where it listens, and the session's configuration. `listen` is
    /// ignored: a dedicated server has no player of its own.
    pub host: HostOptions,
    /// Ticks a second: each one frame of the scene, its fixed steps, and
    /// one receive and one flush of the network. At least 1.
    pub tick_rate: u32,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            host: HostOptions::default(),
            tick_rate: DEFAULT_TICK_RATE,
        }
    }
}

/// Why a server could not start.
#[derive(Debug)]
pub enum ServerError {
    /// The scene would not load.
    Scene(SceneError),
    /// The ports would not open.
    Session(SessionError),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scene(e) => write!(f, "the scene would not load: {e}"),
            Self::Session(e) => write!(f, "could not host: {e}"),
        }
    }
}

impl std::error::Error for ServerError {}

/// A headless game hosting a session, stepped one fixed tick at a time.
pub struct Server {
    session: Headless,
    step: Duration,
    ticks: u64,
}

impl Server {
    /// Hosts `app` — built as a game builds it, its ticks and registry
    /// included — with its world as it is.
    pub fn new(app: App, options: &ServerOptions) -> Result<Self, ServerError> {
        Self::host(Headless::new(app), options)
    }

    /// Loads `scene` into `app` (from its asset root) and hosts it.
    pub fn open(app: App, scene: &AssetPath, options: &ServerOptions) -> Result<Self, ServerError> {
        Self::host(
            Headless::open(app, scene).map_err(ServerError::Scene)?,
            options,
        )
    }

    fn host(mut session: Headless, options: &ServerOptions) -> Result<Self, ServerError> {
        let host = HostOptions {
            listen: false,
            ..options.host.clone()
        };
        session
            .net_mut()
            .host(&host)
            .map_err(ServerError::Session)?;
        Ok(Self {
            session,
            step: Duration::from_secs_f64(1.0 / f64::from(options.tick_rate.max(1))),
            ticks: 0,
        })
    }

    /// One tick: a frame of the scene a step long — the network received,
    /// the game's ticks, the physics, the network flushed. The joins and
    /// leaves it saw are logged and in [`events`](Self::events).
    pub fn tick(&mut self) {
        self.session.step(1, self.step.as_secs_f32());
        self.ticks += 1;
        for event in self.session.net().events() {
            match event {
                NetEvent::Connected(peer) => log::info!("{peer} joined"),
                NetEvent::Disconnected { peer, reason } => log::info!("{peer} left: {reason}"),
            }
        }
    }

    /// Ticks on the wall clock, sleeping between, until `keep_going` says
    /// stop — asked after every tick.
    pub fn run(&mut self, mut keep_going: impl FnMut(&mut Self) -> bool) {
        let start = Instant::now();
        let mut pacer = Pacer::new(self.step);
        loop {
            for _ in 0..pacer.due(start.elapsed()) {
                self.tick();
                if !keep_going(self) {
                    return;
                }
            }
            std::thread::sleep(pacer.wait(start.elapsed()));
        }
    }

    /// Ticks on the wall clock for `duration`.
    pub fn run_for(&mut self, duration: Duration) {
        let start = Instant::now();
        self.run(|_| start.elapsed() < duration);
    }

    /// What the last tick's receive brought: joins, leaves, kicks,
    /// timeouts.
    pub fn events(&self) -> &[NetEvent] {
        self.session.net().events()
    }

    /// The clients connected now.
    pub fn clients(&self) -> Vec<PeerId> {
        self.session.net().peers().collect()
    }

    /// Removes a client, telling it it was kicked; the server sees the
    /// leave in the next tick's events.
    pub fn kick(&mut self, peer: PeerId) {
        self.session.net_mut().kick(peer);
    }

    /// Where clients reach the server: one address per transport.
    pub fn addresses(&self) -> Vec<Address> {
        self.session
            .net()
            .endpoint()
            .map(|e| e.local_addrs())
            .unwrap_or_default()
    }

    /// Ticks run so far.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// How long a tick is.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// The headless game underneath: its world, its network session.
    pub fn headless(&self) -> &Headless {
        &self.session
    }

    /// The headless game underneath, mutably.
    pub fn headless_mut(&mut self) -> &mut Headless {
        &mut self.session
    }

    /// Tells every client the server is shutting down and stops hosting;
    /// dropping the server does the same.
    pub fn shut_down(&mut self) {
        self.session.net_mut().stop();
        // One more tick hands out the goodbyes and lets the session go.
        self.tick();
    }
}
