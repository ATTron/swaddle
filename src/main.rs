use config::{Config, File};
use dbus::{
    arg::messageitem::MessageItem,
    blocking::{BlockingSender, Connection},
    Message,
};
use env_logger::Env;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::{self, create_dir_all},
    path::PathBuf,
    process::{Child, Command},
    thread::sleep,
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize, Serialize)]
struct Settings {
    debug: bool,
    server: ServerSettings,
}

#[derive(Debug, Deserialize, Serialize)]
struct ServerSettings {
    inhibit_duration: u64,
    sleep_duration: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            debug: false,
            server: ServerSettings {
                inhibit_duration: 25,
                sleep_duration: 5,
            },
        }
    }
}

struct IdleApp {
    conn: Connection,
    process_running: bool,
    should_block: bool,
    inhibit_process: Option<Child>,
    last_block_time: Option<Instant>,
    config: Settings,
}

impl IdleApp {
    fn new(config_from_file: Result<Settings, Box<dyn std::error::Error>>) -> IdleApp {
        let conn = Connection::new_session().expect("Failed to connect to D-Bus");
        let mut config: Settings = Settings {
            debug: false,
            server: ServerSettings {
                inhibit_duration: 25,
                sleep_duration: 5,
            },
        };
        match config_from_file {
            Ok(file_config) => config = file_config,
            Err(_e) => {
                log::debug!("No config found or parsed. Using the defaults");
            }
        }
        IdleApp {
            conn,
            process_running: false,
            should_block: false,
            inhibit_process: None::<Child>,
            last_block_time: None,
            config,
        }
    }

    // We want to check every single media player to see if they are playing
    fn list_media_players(&self) -> Result<Vec<String>, dbus::Error> {
        let msg = Message::new_method_call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "ListNames",
        )
        .map_err(|e| dbus::Error::new_failed(&e.to_string()))?;

        let response = self
            .conn
            .send_with_reply_and_block(msg, Duration::from_secs(5))?;
        let names: Vec<String> = response
            .get1()
            .ok_or_else(|| dbus::Error::new_failed("Failed to get names from response"))?;

        Ok(names
            .into_iter()
            .filter(|name| name.starts_with("org.mpris.MediaPlayer2."))
            .collect())
    }

    fn check_playback_status(&mut self) -> Result<(), Box<dyn Error>> {
        let players = self.list_media_players()?;

        log::debug!("Listing players! {:?}", players);
        if players.len() <= 0 && self.process_running {
            self.should_block = false;
            return Ok(());
        }
        for service in players {
            let object_path = "/org/mpris/MediaPlayer2";
            let interface = "org.mpris.MediaPlayer2.Player";
            let property = "PlaybackStatus";

            let msg = Message::new_method_call(
                service,
                object_path,
                "org.freedesktop.DBus.Properties",
                "Get",
            )
            .unwrap()
            .append1(interface)
            .append1(property);

            let response = self
                .conn
                .send_with_reply_and_block(msg, Duration::from_secs(5));

            log::debug!("response is {:?}", response);
            match response {
                Ok(resp) => {
                    if let Some(arg) = resp.get_items().get(0) {
                        match arg {
                            MessageItem::Variant(ref value) => match **value {
                                MessageItem::Str(ref s) => {
                                    if s == "Playing" {
                                        self.should_block = true;
                                        break;
                                    }
                                    self.should_block = false;
                                }
                                _ => log::debug!("Not a string inside the variant. . . IDK what to do so I will throw it away. It is a {:?}", value),
                            },
                            _ => {
                                log::debug!(
                                    "Not a Variant . . . IDK what to do so I will throw it away. It is a {:?}", arg
                                );
                            }
                        }
                    } else {
                        log::debug!("No arguments found in the message.");
                    }
                }
                Err(_) => {
                    log::error!("Unable to lookup playback . . . skipping");
                }
            }
        }
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let mut next_check = Instant::now();
        self.last_block_time = Some(Instant::now());
        loop {
            let _ = self.check_playback_status();
            log::debug!(
                "should_block: {:?} -- process_running: {:?}",
                self.should_block,
                self.process_running
            );
            if Instant::now() >= next_check {
                log::debug!("timing check");
                if self.should_block && !self.process_running {
                    let _ = self.check_and_kill_zombies();
                    match self.run_cmd() {
                        Ok(child) => {
                            log::debug!("Swayidle is inhibiting now!");
                            self.inhibit_process = Some(child);
                            next_check = next_check
                                + Duration::from_secs(self.config.server.inhibit_duration);
                        }
                        Err(e) => {
                            eprintln!("unable to block swayidle :: {:?}", e)
                        }
                    }
                } else if !self.should_block {
                    let _ = self.check_and_kill_zombies();
                    self.process_running = false;
                } else {
                    let _ = self.check_and_kill_zombies();
                    self.process_running = false;
                }
            }
            if self.should_block && self.process_running {
                sleep(Duration::from_secs(
                    self.config.server.sleep_duration + self.config.server.inhibit_duration,
                ))
            } else {
                sleep(Duration::from_secs(self.config.server.sleep_duration));
            }
        }
    }

    fn check_and_kill_zombies(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(ref mut killing) = self.inhibit_process.take() {
            log::debug!("Killing the child process");
            killing.wait()?;
            killing.kill()?;
            match killing.try_wait() {
                Ok(None) => {
                    log::debug!("Zombie Detected 🧟: Killing now");
                    killing.wait()?;
                    killing.kill()?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn run_cmd(&mut self) -> Result<Child, Box<dyn Error>> {
        match Command::new("systemd-inhibit")
            .arg("--what")
            .arg("idle")
            .arg("--who")
            .arg("swayidle-inhibit")
            .arg("--why")
            .arg("audio playing")
            .arg("--mode")
            .arg("block")
            .arg("sh")
            .arg("-c")
            .arg(format!("sleep {}", self.config.server.inhibit_duration))
            .spawn()
        {
            Ok(child) => {
                log::debug!("systemd-inhibit has been spawned");
                self.last_block_time = Some(Instant::now());
                self.process_running = true;
                Ok(child)
            }
            Err(e) => {
                log::error!("Failed to execute systemd-inhibit command: {:?}", e);
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unable to block swayidle due to unknown error",
                )))
            }
        }
    }
}

fn get_config_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Could not find home directory");
    path.push(".config/swaddle/config.toml");
    path
}

fn read_or_create_config() -> Result<Settings, Box<dyn std::error::Error>> {
    let config_path = get_config_path();

    if !config_path.exists() {
        let default_settings = Settings::default();
        let config_dir = config_path.parent().unwrap();
        create_dir_all(config_dir)?;
        let _ = fs::write(
            &config_path,
            toml::to_string_pretty(&default_settings).unwrap(),
        );
        return Ok(default_settings);
    }

    Ok(Config::builder()
        .add_source(File::from(config_path))
        .build()?
        .try_deserialize()?)
}

fn main() {
    let config = read_or_create_config();
    let mut app = IdleApp::new(config);
    let log_level = if app.config.debug { "debug" } else { "info" };

    env_logger::Builder::from_env(Env::default().default_filter_or(log_level)).init();

    log::debug!("Swaddle starting up");

    let _ = app.run();
}
