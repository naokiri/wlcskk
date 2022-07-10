# Dev notes
## wayland input method basics
https://dcz_self.gitlab.io/posts/input_method/

## This project
Targetting to use with [swaywm](https://github.com/swaywm), which is on [wlroots](https://gitlab.freedesktop.org/wlroots/wlroots).

So, this will implement the protocol versions used in `wlroot`, which means input-method-unstable-v2 and virtual-keyboard-unstable-v1 protocols.

This program should ask to grab the keyboard, and then send the text using input-method protocol or send key events using virtual-keyboard protocol.


## 
WAYLAND_DEBUG=1

For the time being this is using `env_logger`. Can use env var like RUST_LOG=warn,info,debug 