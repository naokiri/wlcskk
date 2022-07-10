use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::{DispatchData, Main};
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::Event as KeyEvent;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::Event as IMEvent;
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;


pub(crate) struct WlCskkContext {
    vk: Main<ZwpVirtualKeyboardV1>,
    im: Main<ZwpInputMethodV2>,
    keymap_init_done: bool,
}

impl Drop for WlCskkContext {
    fn drop(&mut self) {
        self.im.destroy();
    }
}

// Global state used among wayland callbacks
impl WlCskkContext {
    fn delegate_key(&self, time: u32, key: u32, state: KeyState) {
        if self.keymap_init_done {
            self.vk.key(time, key, state as _);
        }
    }

    fn delegate_modifier(
        &self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        if self.keymap_init_done {
            self.vk
                .modifiers(mods_depressed, mods_latched, mods_locked, group);
        }
    }

    // https://docs.rs/wayland-protocols/latest/wayland_protocols/misc/zwp_input_method_v2/client/zwp_input_method_v2/enum.Event.html
    pub fn handle_im_ev(&mut self, im_event: IMEvent) {
        log::error!("IMEvent: {:?}", im_event);
        match im_event {
            IMEvent::Activate => {}
            IMEvent::Deactivate => {}
            IMEvent::Unavailable => {
                // TODO: How to die when on this event?
                // panic! is definitely not good. Just for time being during development.
                // We have to make sure this daemon doesn't hold wayland resources, especially keyboard grab, and then die.
                self.im.destroy();
                panic!("Unavailable")
            }
            IMEvent::SurroundingText { .. } => {}
            IMEvent::TextChangeCause { .. } => {}
            IMEvent::ContentType { hint, purpose, .. } => {}
            IMEvent::Done => {}
            _ => {}
        }
    }

    // https://docs.rs/wayland-protocols/latest/wayland_protocols/misc/zwp_input_method_v2/client/zwp_input_method_keyboard_grab_v2/enum.Event.html
    pub fn handle_key_ev(&mut self, key_event: KeyEvent) {
        log::error!("KeyEvent: {:?}", key_event);
        match key_event {
            KeyEvent::Keymap { format, fd, size } => {
                if !self.keymap_init_done {
                    self.vk.keymap(format as _, fd, size);
                }
                self.keymap_init_done = true;
            }
            KeyEvent::RepeatInfo { .. } => {
                log::error!("Grabbed.")
            }
            KeyEvent::Key {
                state, key, time, ..
            } => {
                self.delegate_key(time, key, state);
            }
            KeyEvent::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                self.delegate_modifier(mods_depressed, mods_latched, mods_locked, group);
            }
            _ => {}
        }
    }

    pub fn new(vk: Main<ZwpVirtualKeyboardV1>, im: Main<ZwpInputMethodV2>) -> Self {
        WlCskkContext {
            vk,
            im,
            keymap_init_done: false,
        }
    }

    pub fn from_wayland_data<'a>(data: &'a mut DispatchData) -> &'a mut Self {
        data.get::<Self>().unwrap()
    }
}
