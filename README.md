# Swaddle

**NOTE: This Project Has Been Moved To [Codeberg](https://codeberg.org/templetr0n)**

Swayidle inhibitor with automatic detection for audio / video and prevent your system from sleeping.

## Overview

The main function of this project is to keep any sway based WM from going into an idle state when consuming media. Swaddle will monitor the dbus running daemon and based on values it sees in `Playback Status` will correctly cause idling or inhibition. The idea is to remove the need for a dedicate button in your swaybar config to enable and disable the inhibiting of swayidle.

## Installation

AUR:
```bash
paru -S swaddle
```

### Building from source

* Clone the repo and execute

   ```bash
   just build_release
   ```

* You can move the binary into your `$PATH` or run directly

## Post-Install

To integrate swaddle with Sway/Hyprland/River, add the following line to your Sway/Hypr configuration:

* Sway:

```conf
# Swaddle configuration
exec_always --no-startup-id /usr/local/bin/swaddle &
```

* Hyprland:

```conf
# Swaddle configuration
exec = /usr/local/bin/swaddle &
```

Then reload your configuration or restart Sway/Hyprland.

### Configuration File (Optional)

The first time swaddle is run it will create a config file 
under `$HOME/.config/swaddle/config.toml`.

You can also create / overwrite the config with the following options  

| Name | Value | Explaination | Default |
| ---- | ----- | ------------ | ------- |
|debug|boolean|should swaddle be run in debug mode|<span style="color:grey">false</span>|
|server|table|includes the options to tweak how swaddle operates||
|server.inhibit_duration|integer|number of seconds to inhibit per cycle|<span style="color:grey">25</span>|
|server.sleep_duration|integer|number of seconds to wait between checks|<span style="color:grey">5</span>|

---
