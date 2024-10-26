use dbus::{
    arg::{messageitem::MessageItem, RefArg, Variant},
    blocking::{BlockingSender, Connection},
    message::MatchRule,
    Message,
};
use env_logger::Env;
use log::warn;
use std::{
    collections::HashMap,
    error::Error,
    process::{Child, Command},
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::sleep,
    time::{Duration, Instant},
};

struct IdleApp {
    conn: Connection,
    inhibit_duration: u64,
    process_running: AtomicBool,
    should_block: AtomicBool,
}

impl IdleApp {
    fn new(inhibit_duration: u64) -> IdleApp {
        let conn = Connection::new_session().expect("Failed to connect to D-Bus");
        IdleApp {
            conn,
            inhibit_duration,
            process_running: AtomicBool::new(false),
            should_block: AtomicBool::new(false),
        }
    }

    fn check_playback_status(&self) {
        let service = "org.mpris.MediaPlayer2.firefox.instance_1_41";
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

        match response {
            Ok(resp) => {
                if let Some(arg) = resp.get_items().get(0) {
                    // Match on the MessageItem to see if it is a Variant
                    println!("ARG IS {:?}", arg);
                    match arg {
                        MessageItem::Variant(ref value) => match **value {
                            MessageItem::Str(ref s) => {
                                println!("showing unwrapped: {}", s);
                                if s == "Playing" {
                                    self.should_block
                                        .store(true, std::sync::atomic::Ordering::SeqCst);
                                }
                                if s == "Paused" {
                                    self.should_block
                                        .store(false, std::sync::atomic::Ordering::SeqCst);
                                }
                            }
                            _ => println!("Not a string variant inside Variant"),
                        },
                        _ => {
                            println!("Not a Variant");
                        }
                    }
                } else {
                    println!("No arguments found in the message.");
                }
            }
            Err(_) => {
                eprintln!("Unable to lookup playback currently...skipping");
            }
        }
    }

    fn run(&self) {
        loop {
            self.check_playback_status();
            sleep(Duration::from_millis(5000));
        }
    }
}

// fn run(&self) -> Result<(), Box<dyn Error>> {
//     if let Err(e) = self.dbus_runner.add_match() {
//         eprintln!("Unable to setup dbus_runner :: {:?}", e);
//     }
//     let mut next_check = Instant::now();
//     loop {
//         let current_time = Instant::now();
//         if current_time >= next_check {
//             if let Ok(mut inhibit) = self.inhibit_process.lock() {
//                 if inhibit.0.is_none() || current_time >= inhibit.1 {
//                     let process_running = Arc::clone(&self.process_running);
//                     if let Some(mut child) = inhibit.0.take() {
//                         child.wait()?;
//                         let _ = child.kill();
//                         process_running.store(false, std::sync::atomic::Ordering::SeqCst);
//                     }
//                     match self
//                         .dbus_runner
//                         .connection
//                         .process(Duration::from_millis(1000))
//                     {
//                         Ok(_) => {
//                             let block = self
//                                 .dbus_runner
//                                 .good_to_send
//                                 .load(std::sync::atomic::Ordering::SeqCst);
//                             // Only spawn a single child if its blocking already, move on
//                             log::debug!(
//                                 "should_block = {} - process_running = {:?}",
//                                 block,
//                                 process_running,
//                             );
//                             if block && !process_running.load(std::sync::atomic::Ordering::SeqCst) {
//                                 if let Some(mut killing) = inhibit.0.take() {
//                                     killing.wait()?;
//                                     killing.kill()?;
//                                 }
//                                 match self.run_cmd() {
//                                     Ok(child) => {
//                                         log::debug!("Swayidle is inhibiting now!");
//                                         inhibit.0 = Some(child);
//                                     }
//                                     Err(e) => {
//                                         eprintln!("unable to block swayidle :: {:?}", e)
//                                     }
//                                 }
//                             } else if !block {
//                                 if let Some(mut killing) = inhibit.0.take() {
//                                     killing.wait()?;
//                                     killing.kill()?;
//                                     process_running
//                                         .store(false, std::sync::atomic::Ordering::SeqCst);
//                                 }
//                             }
//                         }
//                         Err(e) => eprintln!("Error handling D-Bus message: {:?}", e),
//                     }
//                     inhibit.1 =
//                         current_time + Duration::from_secs(INHIBIT_DURATION - OVERLAP_DURATION);
//
//                     next_check = current_time + Duration::from_secs(OVERLAP_DURATION);
//                 }
//             }
//         }
//         sleep(Duration::from_millis(5000));
//     }
// }

// fn run_cmd() -> Result<Child, Box<dyn Error>> {
//     log::debug!("command is spawning");
//     let process_running = Arc::clone(&self.process_running);
//     match Command::new("systemd-inhibit")
//         .arg("--what")
//         .arg("idle")
//         .arg("--who")
//         .arg("swayidle-inhibit")
//         .arg("--why")
//         .arg("audio playing")
//         .arg("--mode")
//         .arg("block")
//         .arg("sh")
//         .arg("-c")
//         .arg(format!("sleep {}", self.inhibit_duration))
//         .spawn()
//     {
//         Ok(child) => {
//             if let Ok(mut last_block_time) = self.last_block_time.lock() {
//                 *last_block_time = Some(Instant::now());
//             }
//             process_running.store(true, std::sync::atomic::Ordering::SeqCst);
//             Ok(child)
//         }
//         Err(e) => {
//             eprintln!("Failed to execute systemd-inhibit command: {:?}", e);
//             Err(Box::new(std::io::Error::new(
//                 std::io::ErrorKind::Other,
//                 "Unable to block swayidle due to unknown error",
//             )))
//         }
//     }
// }

// const INTERFACE_NAME: &str = "org.freedesktop.DBus.Properties";
// const DBUS_NAMESPACE: &str = "/org/mpris/MediaPlayer2";
const INHIBIT_DURATION: u64 = 25;
// const OVERLAP_DURATION: u64 = 5;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    log::debug!("Swaddle starting up");

    let app = IdleApp::new(INHIBIT_DURATION);
    app.run();
}
