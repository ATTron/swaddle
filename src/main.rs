use dbus::{
    arg::messageitem::MessageItem,
    blocking::{BlockingSender, Connection},
    Message,
};
use env_logger::Env;
use std::{
    error::Error,
    process::{Child, Command},
    time::Instant,
};
use std::{thread::sleep, time::Duration};

struct IdleApp {
    conn: Connection,
    inhibit_duration: u64,
    process_running: bool,
    should_block: bool,
    inhibit_process: Option<Child>,
    last_block_time: Option<Instant>,
}

impl IdleApp {
    fn new(inhibit_duration: u64) -> IdleApp {
        let conn = Connection::new_session().expect("Failed to connect to D-Bus");
        IdleApp {
            conn,
            inhibit_duration,
            process_running: false,
            should_block: false,
            inhibit_process: None::<Child>,
            last_block_time: None,
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
                        log::debug!("ARG IS {:?}", arg);
                        match arg {
                            MessageItem::Variant(ref value) => match **value {
                                MessageItem::Str(ref s) => {
                                    log::debug!("showing unwrapped: {}", s);
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
                log::debug!("hey we made it into the timing check");
                if self.should_block && !self.process_running {
                    log::debug!("HEY WE SHOULD BLOCK");
                    if let Some(mut killing) = self.inhibit_process.take() {
                        log::debug!("Killing the child process");
                        killing.wait()?;
                        killing.kill()?;
                    }
                    match self.run_cmd() {
                        Ok(child) => {
                            log::debug!("Swayidle is inhibiting now!");
                            self.inhibit_process = Some(child);
                            next_check = next_check + Duration::from_secs(self.inhibit_duration);
                        }
                        Err(e) => {
                            eprintln!("unable to block swayidle :: {:?}", e)
                        }
                    }
                } else if !self.should_block {
                    if let Some(ref mut killing) = self.inhibit_process {
                        log::debug!("Killing the child process");
                        killing.wait()?;
                        killing.kill()?;
                        self.process_running = false;
                    }
                }
            }
            if self.should_block && self.process_running {
                sleep(Duration::from_secs(SLEEP_DURATION + INHIBIT_DURATION))
            } else {
                sleep(Duration::from_secs(SLEEP_DURATION));
            }
        }
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
            .arg(format!("sleep {}", self.inhibit_duration))
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

const INHIBIT_DURATION: u64 = 25;
const SLEEP_DURATION: u64 = 5;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    log::debug!("Swaddle starting up");
    log::debug!("Swaddle rewrite version is being called");

    let mut app = IdleApp::new(INHIBIT_DURATION);
    let _ = app.run();
}
