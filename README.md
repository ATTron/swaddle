# README for Swaddle

Swayidle inhibitor that automatically detects audio / video and will prevent your system from sleeping. No manual intervention needed!

**note**  
**right now this has only been tested with firefox, not sure how well it will work with chrome based browsers at this time**

## Installation

Swaddle can be installed from the AUR:

```sh
yay -S swaddle
```

## Post-Install

 To integrate swaddle with Sway/Hyprland, add the following line to your Sway/Hypr configuration:
 `exec_always --no-startup-id /usr/local/bin/swaddle &`
 Then reload your configuration or restart Sway/Hyprland.

## Overview

The main function of this project is to keep swayWM from going into an idle state when watching Youtube videos. This Rust project includes a D-Bus Runner (`DBusRunner`) and an Idle Application (`IdleApp`). It is designed to interface with D-Bus for message handling, particularly for managing media playback statuses, and to execute commands based on these statuses. 

### DBusRunner

`DBusRunner` is responsible for creating a D-Bus session and setting up message match rules for listening to specific D-Bus signals. It plays a critical role in responding to changes in media playback status.

### IdleApp

`IdleApp` utilizes `DBusRunner` to monitor playback status and controls system behavior (like inhibiting system idle actions) based on the playback state. It represents the core logic of the application, managing state and triggering commands as necessary.

## Features

- **D-Bus Interaction:** Listen to D-Bus signals and react to changes in media playback status.
- **Command Execution:** Based on the playback status, execute system commands, specifically using `systemd-inhibit` to control system idle behavior.
- **Concurrency and Synchronization:** Manage shared state and handle concurrency using Rust's `Arc` and `Mutex`.

## Dependencies

- `dbus`: For interfacing with the D-Bus.
- `std`: Standard library, particularly for error handling, synchronization primitives, and process management.

## Setup and Running

1. Ensure Rust and Cargo are installed.
2. Clone the repository.
3. Run the application using Cargo:

   ```bash
   cargo run
   ```

## Testing

The project includes unit tests for both `DBusRunner` and `IdleApp`. To run these tests, use:

```bash
cargo test
```

### DBusRunner Tests

- **Initialization Test:** Ensures that a new `DBusRunner` instance is correctly initialized.
- **Add Match Test:** Tests the ability to add a match rule to the D-Bus connection.

### IdleApp Tests

- **Initialization Test:** Confirms that `IdleApp` initializes correctly with the given inhibit duration.

### CommandCaller Tests

- **Mock Implementation:** A mock implementation of `CommandCaller` is provided for testing command execution.

## Architecture

- **Traits (`DBusInterface`, `CommandCaller`):** Define the contract for D-Bus interactions and command execution.
- **Structs (`DBusRunner`, `IdleApp`):** Implement the application logic, managing D-Bus sessions, and reacting to playback status.

## Error Handling

Errors are managed using Rust's standard `Result` and `Error` types, ensuring robust and clear error handling throughout the application.

## Future Enhancements

- Extend the command execution capabilities based on additional playback statuses or other D-Bus signals.
- Integrate with more complex system behaviors or external applications.
- Improve error handling and logging for better diagnostics and maintenance.
- Add more unit tests and integration tests
- Chrome testing / support
- Make a service / add to AUR

---
