# README for Swaddle

Swayidle inhibitor that automatically detects audio / video and will prevent your system from sleeping. No manual intervention needed!

**note**  
**right now this has only been tested with zen (firefox) and brave(chrome)**

## Installation

Swaddle can be installed from the AUR:

```sh
paru -S swaddle
```

### Building from source

* Clone the repo and execute

   ```sh
   cargo build --release
   ```

* You can move the binary into your `$PATH` or run directly

#### Debugging

To get some debugging logging from swaddle you can set the log level to debug and execute

```sh
RUST_LOG=debug ./target/release/swaddle
```

## Post-Install

 To integrate swaddle with Sway/Hyprland, add the following line to your Sway/Hypr configuration:
 `exec_always --no-startup-id /usr/local/bin/swaddle &`
 Then reload your configuration or restart Sway/Hyprland.

## Overview

The main function of this project is to keep any sway based WM from going into an idle state when consuming media. Swaddle will monitor the dbus running daemon and based on values it sees in `Playback Status` will correctly cause idling or inhibition.


## Dependencies

* `dbus`: For interfacing with the D-Bus.
* `std`: Standard library, particularly for error handling, synchronization primitives, and process management.

## Future Enhancements

* Extend the command execution capabilities based on additional playback statuses or other D-Bus signals.
* Integrate with more complex system behaviors or external applications.
* Improve error handling and logging for better diagnostics and maintenance.
* Add more unit tests and integration tests

---
