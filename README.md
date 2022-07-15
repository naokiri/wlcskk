# WLCSKK
Wayland text input daemon that implements Wayland ZwpInputMethodV2 and ZwpVirtualKeyboardV1 partially. 

Targetting to use under swaywm, aiming to follow the wlroot applicable protocols.

Note that those protocols are unstable yet.

## Current Status
Works, buggy.

## How to try
First, stop all other daemons that grabs keyboard. 

Then, run 

    $ cargo run

Once you run this daemon, default config file will be generated and you can specify the skk dictionary files.
Config file defaults to `~/.config/wlcskk/wlcskk.toml`

    $ RUST_LOG=debug cargo run 2> error.txt 

## Copyright

Copyright (C) 2022 Naoaki Iwakiri

This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
