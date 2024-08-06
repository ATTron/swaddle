use dbus::{
    arg::{RefArg, Variant},
    blocking::Connection,
    message::MatchRule,
};
use env_logger::Env;
use std::{
    collections::HashMap,
    error::Error,
    process::{Child, Command},
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::sleep,
    time::{Duration, Instant},
};

trait DBusInterface {
    fn add_match(&self) -> Result<(), Box<dyn Error>>;
}

struct DBusRunner {
    connection: Arc<Connection>,
    good_to_send: Arc<AtomicBool>,
}

impl DBusRunner {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let connection = Connection::new_session()?;
        Ok(DBusRunner {
            connection: Arc::new(connection),
            good_to_send: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl DBusInterface for DBusRunner {
    fn add_match(&self) -> Result<(), Box<dyn Error>> {
        let rule = MatchRule::new()
            .with_interface(INTERFACE_NAME)
            .with_namespaced_path(DBUS_NAMESPACE);

        let good_to_send = Arc::clone(&self.good_to_send);
        self.connection.add_match(rule, move |(), _, msg| {
            let items: HashMap<String, Variant<Box<dyn RefArg>>> =
                msg.read3::<String, HashMap<_, _>, Vec<String>>().unwrap().1;
            if let Some(playback_status) = items.get("PlaybackStatus") {
                if let Some(status) = playback_status.0.as_str() {
                    log::debug!("Status found {}", status);
                    if status == "Playing" {
                        good_to_send.store(true, std::sync::atomic::Ordering::SeqCst);
                    } else {
                        good_to_send.store(false, std::sync::atomic::Ordering::SeqCst);
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
    process_running: Arc<AtomicBool>,
}

impl IdleApp {
    fn new(inhibit_duration: i64) -> Result<Self, Box<dyn Error>> {
        let dbus_runner = DBusRunner::new()?;
        Ok(IdleApp {
            dbus_runner,
            inhibit_duration,
            last_block_time: Arc::new(Mutex::new(None)),
            inhibit_process: Arc::new(Mutex::new((None::<Child>, Instant::now()))),
            process_running: Arc::new(AtomicBool::new(false)),
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
                        let process_running = Arc::clone(&self.process_running);
                        if let Some(mut child) = inhibit.0.take() {
                            child.wait()?;
                            let _ = child.kill();
                            process_running.store(false, std::sync::atomic::Ordering::SeqCst);
                        }
                        match self
                            .dbus_runner
                            .connection
                            .process(Duration::from_millis(1000))
                        {
                            Ok(_) => {
                                let block = self
                                    .dbus_runner
                                    .good_to_send
                                    .load(std::sync::atomic::Ordering::SeqCst);
                                // Only spawn a single child if its blocking already, move on
                                log::debug!(
                                    "should_block = {} - process_running = {:?}",
                                    block,
                                    process_running,
                                );
                                if block
                                    && !process_running.load(std::sync::atomic::Ordering::SeqCst)
                                {
                                    if let Some(mut killing) = inhibit.0.take() {
                                        killing.wait()?;
                                        killing.kill()?;
                                    }
                                    match self.run_cmd() {
                                        Ok(child) => {
                                            log::debug!("Swayidle is inhibiting now!");
                                            inhibit.0 = Some(child);
                                        }
                                        Err(e) => {
                                            eprintln!("unable to block swayidle :: {:?}", e)
                                        }
                                    }
                                } else if !block {
                                    if let Some(mut killing) = inhibit.0.take() {
                                        killing.wait()?;
                                        killing.kill()?;
                                        process_running
                                            .store(false, std::sync::atomic::Ordering::SeqCst);
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
        log::debug!("command is spawning");
        let process_running = Arc::clone(&self.process_running);
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
                process_running.store(true, std::sync::atomic::Ordering::SeqCst);
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
const INHIBIT_DURATION: u64 = 25;
const OVERLAP_DURATION: u64 = 5;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    log::debug!("Swaddle starting up");
    let app = IdleApp::new(30)?;
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
