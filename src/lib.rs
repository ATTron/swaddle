use config::{Config, File};
use dbus::{
    arg::messageitem::MessageItem,
    blocking::{BlockingSender, Connection},
    Message,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::{self, create_dir_all},
    path::PathBuf,
    process::{Child, Command},
    time::Duration,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub debug: bool,
    pub server: ServerSettings,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerSettings {
    pub inhibit_duration: u64,
    pub sleep_duration: u64,
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

pub struct IdleApp {
    pub conn: Connection,
    pub inhibit_process: Option<Child>,
    pub config: Settings,
}

impl IdleApp {
    pub fn new(config_from_file: Result<Settings, Box<dyn std::error::Error>>) -> IdleApp {
        let conn = Connection::new_session().expect("Failed to connect to D-Bus");
        let config = config_from_file
            .inspect_err(|_| log::debug!("No config found or parsed. Using the defaults"))
            .unwrap_or_default();
        IdleApp {
            conn,
            inhibit_process: None,
            config,
        }
    }

    // We want to check every single media player to see if they are playing
    pub fn list_media_players(&self) -> Result<Vec<String>, dbus::Error> {
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

    pub fn check_playback_status(&self) -> bool {
        let players = match self.list_media_players() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to list media players: {:?}", e);
                return false;
            }
        };

        log::debug!("Listing players: {:?}", players);

        for service in players {
            let object_path = "/org/mpris/MediaPlayer2";
            let interface = "org.mpris.MediaPlayer2.Player";
            let property = "PlaybackStatus";

            let msg = match Message::new_method_call(
                service,
                object_path,
                "org.freedesktop.DBus.Properties",
                "Get",
            ) {
                Ok(m) => m.append1(interface).append1(property),
                Err(e) => {
                    log::error!("Failed to create D-Bus message: {:?}", e);
                    continue;
                }
            };

            let response = self
                .conn
                .send_with_reply_and_block(msg, Duration::from_secs(5));

            log::debug!("Connection Message: {:?}", response);
            match response {
                Ok(resp) => {
                    let items = resp.get_items();
                    let Some(arg) = items.first() else {
                        log::debug!("No arguments found in the message.");
                        continue;
                    };

                    let MessageItem::Variant(ref value) = arg else {
                        log::debug!("IDK what to do...throwing away {:?}", arg);
                        continue;
                    };

                    let MessageItem::Str(ref s) = **value else {
                        log::debug!(
                            "No string inside the variant. . . . throwing away {:?}",
                            value
                        );
                        continue;
                    };

                    if s == "Playing" {
                        return true;
                    }
                }
                Err(_) => {
                    log::error!("Failed to lookup playback . . . skipping");
                }
            }
        }
        false
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let should_block = self.check_playback_status();
            log::debug!(
                "should_block: {:?} -- process_running: {:?}",
                should_block,
                self.inhibit_process.is_some()
            );

            if should_block {
                let mut needs_spawn = true;
                if let Some(ref mut child) = self.inhibit_process {
                    match child.try_wait() {
                        Ok(None) => needs_spawn = false,
                        _ => {}
                    }
                }

                if needs_spawn {
                    let _ = self.check_and_kill_zombies();
                    if let Ok(child) = self.run_cmd() {
                        log::debug!("Swayidle is inhibiting now!");
                        self.inhibit_process = Some(child);
                    }
                }
            } else if self.inhibit_process.is_some() {
                let _ = self.check_and_kill_zombies();
            }

            std::thread::sleep(Duration::from_secs(self.config.server.sleep_duration));
        }
    }

    pub fn check_and_kill_zombies(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(mut killing) = self.inhibit_process.take() {
            log::debug!("Killing the child process");
            let _ = killing.kill();
            let _ = killing.wait();
        }
        Ok(())
    }

    pub fn run_cmd(&mut self) -> Result<Child, Box<dyn Error>> {
        Command::new("systemd-inhibit")
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
            .inspect(|_| log::debug!("systemd-inhibit has been spawned"))
            .map_err(|e| {
                log::error!("Failed to execute systemd-inhibit command: {:?}", e);
                Box::from(std::io::Error::other(
                    "Unable to block swayidle due to unknown error",
                ))
            })
    }
}

fn get_config_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Could not find home directory");
    path.push(".config/swaddle/config.toml");
    path
}

pub fn read_or_create_config() -> Result<Settings, Box<dyn std::error::Error>> {
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
