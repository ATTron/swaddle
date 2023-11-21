use dbus::{
    arg::{RefArg, Variant},
    blocking::Connection,
    message::MatchRule,
};
use std::{
    collections::HashMap,
    error::Error,
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread::sleep,
    time::{Duration, Instant},
};

trait DBusInterface {
    fn add_match(&self) -> Result<(), Box<dyn Error>>;
}

trait CommandCaller {
    fn execute_command(&self) -> Result<(), Box<dyn Error>>;
}
struct DBusRunner {
    connection: Arc<Connection>,
    good_to_send: Arc<Mutex<bool>>,
}

impl DBusRunner {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let connection = Connection::new_session()?;
        Ok(DBusRunner {
            connection: Arc::new(connection),
            good_to_send: Arc::new(Mutex::new(false)),
        })
    }
}

impl DBusInterface for DBusRunner {
    fn add_match(&self) -> Result<(), Box<dyn Error>> {
        let rule = MatchRule::new()
            .with_interface(INTERFACE_NAME)
            .with_namespaced_path(DBUS_NAMESPACE);

        let sending_clone = Arc::clone(&self.good_to_send);
        self.connection.add_match(rule, move |_: (), _, msg| {
            let items: HashMap<String, Variant<Box<dyn RefArg>>> =
                msg.read3::<String, HashMap<_, _>, Vec<String>>().unwrap().1;
            if let Some(playback_status) = items.get("PlaybackStatus") {
                if let Some(status) = playback_status.0.as_str() {
                    if status == "Paused" {
                        if let Ok(mut send_it) = sending_clone.lock() {
                            *send_it = false;
                        }
                    }
                    if status == "Playing" {
                        if let Ok(mut send_it) = sending_clone.lock() {
                            *send_it = true;
                        }
                    }
                }
            }
            true
        })?;
        Ok(())
    }
}

struct IdleApp {
    dbus_runner: DBusRunner,
    inhibit_duration: i64,
    last_block_time: Arc<Mutex<Option<Instant>>>,
    inhibit_process: Arc<Mutex<(Option<Child>, Instant)>>,
}

impl IdleApp {
    fn new(inhibit_duration: i64) -> Result<Self, Box<dyn Error>> {
        let dbus_runner = DBusRunner::new()?;
        Ok(IdleApp {
            dbus_runner,
            inhibit_duration,
            last_block_time: Arc::new(Mutex::new(None)),
            inhibit_process: Arc::new(Mutex::new((None::<Child>, Instant::now()))),
        })
    }

    fn run(&self) -> Result<(), Box<dyn Error>> {
        if let Err(e) = self.dbus_runner.add_match() {
            eprintln!("Unable to setup dbus_runner :: {:?}", e);
        }
        let mut next_check = Instant::now();
        loop {
            let current_time = Instant::now();
            if current_time >= next_check {
                if let Ok(mut inhibit) = self.inhibit_process.lock() {
                    if inhibit.0.is_none() || current_time >= inhibit.1 {
                        if let Some(mut child) = inhibit.0.take() {
                            let _ = child.kill();
                        }
                        match self
                            .dbus_runner
                            .connection
                            .process(Duration::from_millis(1000))
                        {
                            Ok(_) => {
                                if let Ok(block) = self.dbus_runner.good_to_send.lock() {
                                    if *block {
                                        match self.run_cmd() {
                                            Ok(child) => inhibit.0 = Some(child),
                                            Err(e) => {
                                                eprintln!("unable to block swayidle :: {:?}", e)
                                            }
                                        }
                                    } else if !*block {
                                        if let Some(mut killing) = inhibit.0.take() {
                                            let _ = killing.kill();
                                        }
                                    }
                                }
                            }
                            Err(e) => eprintln!("Error handling D-Bus message: {:?}", e),
                        }
                        inhibit.1 =
                            current_time + Duration::from_secs(INHIBIT_DURATION - OVERLAP_DURATION);

                        next_check = current_time + Duration::from_secs(OVERLAP_DURATION);
                    }
                }
            }
            sleep(Duration::from_millis(5000));
        }
    }

    fn run_cmd(&self) -> Result<Child, Box<dyn Error>> {
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
                if let Ok(mut last_block_time) = self.last_block_time.lock() {
                    *last_block_time = Some(Instant::now());
                }
                Ok(child)
            }
            Err(e) => {
                eprintln!("Failed to execute systemd-inhibit command: {:?}", e);
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unable to block swayidle due to unknown error",
                )))
            }
        }
    }
}

const INTERFACE_NAME: &str = "org.freedesktop.DBus.Properties";
const DBUS_NAMESPACE: &str = "/org/mpris/MediaPlayer2";
const INHIBIT_DURATION: u64 = 55;
const OVERLAP_DURATION: u64 = 5;

fn main() -> Result<(), Box<dyn Error>> {
    let app = IdleApp::new(60)?;
    app.run()
}

#[cfg(test)]
mod dbusr_runner_tests {
    use super::*;

    #[test]
    fn test_dbus_runner_initialization() {
        let runner = DBusRunner::new();
        assert!(runner.is_ok());
    }

    #[test]
    fn test_add_match() {
        let runner = DBusRunner::new().unwrap();
        let result = runner.add_match();
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod idle_app_tests {
    use super::*;

    #[test]
    fn test_idle_app_initialization() {
        let app = IdleApp::new(60);
        assert!(app.is_ok());
    }
}

#[cfg(test)]
mod command_caller_tests {
    use super::*;

    struct MockCommandCaller;

    impl CommandCaller for MockCommandCaller {
        fn execute_command(&self) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    #[test]
    fn test_execute_command() {
        let mock_caller = MockCommandCaller;
        let result = mock_caller.execute_command();
        assert!(result.is_ok());
    }
}
