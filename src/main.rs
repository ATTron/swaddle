use env_logger::Env;
use swaddle::{read_or_create_config, IdleApp};

fn main() {
    let config = read_or_create_config();
    let mut app = IdleApp::new(config);
    let log_level = if app.config.debug { "debug" } else { "info" };

    env_logger::Builder::from_env(Env::default().default_filter_or(log_level)).init();

    log::debug!("Swaddle: Starting up . . .");

    let _ = app.run();
}
