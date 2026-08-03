use voltra_core::{App, WindowConfig};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    App::new(WindowConfig {
        title: "Voltra Editor".into(),
        ..Default::default()
    })
    .run();
}
