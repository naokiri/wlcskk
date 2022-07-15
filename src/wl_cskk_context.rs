use crate::app_config::AppConfig;
use cskk::dictionary::CskkDictionary;
use cskk::keyevent::CskkKeyEvent;
use cskk::skk_modes::{CompositionMode, InputMode};
use cskk::CskkContext;
use std::sync::Arc;
use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::protocol::wl_keyboard::KeymapFormat as WlKeymapFormat;
use wayland_client::{DispatchData, Main};
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::Event as KeyEvent;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::Event as IMEvent;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use xkbcommon::xkb::{
    Context, KeyDirection, Keymap, State, CONTEXT_NO_FLAGS, KEYMAP_COMPILE_NO_FLAGS, MOD_NAME_ALT,
    MOD_NAME_CTRL, MOD_NAME_SHIFT, STATE_MODS_EFFECTIVE,
};
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

pub(crate) struct WlCskkContext {
    ctx: CskkContext,
    vk: Main<ZwpVirtualKeyboardV1>,
    im: Main<ZwpInputMethodV2>,
    grab: Option<Main<ZwpInputMethodKeyboardGrabV2>>,
    keymap_init_done: bool,
    keymap_format: Option<WlKeymapFormat>,
    // Next expected state on IM events. IM Done event notifies that we should transition to this next state.
    activating: bool,
    is_active: bool,
    xkb_context: Context,
    xkb_state: Option<State>,
}

impl Drop for WlCskkContext {
    fn drop(&mut self) {
        self.ctx.save_dictionary();
        // im will release corresponding grab if any.
        self.im.destroy();
        self.vk.destroy();
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
            IMEvent::Activate => {
                self.activating = true;
            }
            IMEvent::Deactivate => {
                self.activating = false;
            }
            IMEvent::Unavailable => {
                // TODO: How to die when on this event?
                // panic! is definitely not good. Just for time being during development.
                // We have to send some event to stop the main loop,
                // make sure this daemon doesn't hold wayland resources, especially keyboard grab, and then die.
                self.im.destroy();
                panic!("Unavailable")
            }
            IMEvent::SurroundingText { .. } => {}
            IMEvent::TextChangeCause { .. } => {}
            IMEvent::ContentType {
                hint: _,
                purpose: _,
                ..
            } => {}
            IMEvent::Done => {
                if self.activating && !self.is_active {
                    let grab = self.im.grab_keyboard();
                    log::debug!("Grabbing keyboard");
                    // Handle events using the filters
                    grab.quick_assign(
                        |_handle: Main<ZwpInputMethodKeyboardGrabV2>,
                         ev: KeyEvent,
                         mut data: DispatchData| {
                            let context = WlCskkContext::from_wayland_data(&mut data);
                            context.handle_key_ev(ev)
                        },
                    );
                    self.grab = Some(grab);
                    self.is_active = true;
                } else if !self.activating && self.is_active {
                    if let Some(grab) = &mut self.grab {
                        grab.release();
                    }
                    self.grab = None;
                    self.is_active = false;
                }
            }
            _ => {}
        }
    }

    // https://docs.rs/wayland-protocols/latest/wayland_protocols/misc/zwp_input_method_v2/client/zwp_input_method_keyboard_grab_v2/enum.Event.html
    pub fn handle_key_ev(&mut self, key_event: KeyEvent) {
        log::error!("KeyEvent: {:?}", key_event);
        match key_event {
            KeyEvent::Keymap { format, fd, size } => {
                // This event should be only once? Wayland docs doesn't tell.
                // TODO: Is this rough boolean guard enough?
                if !self.keymap_init_done {
                    self.vk.keymap(format.to_raw(), fd, size);
                    if let Some(keymap) = Keymap::new_from_fd(
                        &self.xkb_context,
                        fd,
                        size as _,
                        format.to_raw(),
                        KEYMAP_COMPILE_NO_FLAGS,
                    ) {
                        self.xkb_state = Some(State::new(&keymap));
                    }
                    self.keymap_format = Some(format);
                }
                self.keymap_init_done = true;
            }
            KeyEvent::RepeatInfo { .. } => {
                log::error!("Grabbed.")
            }
            KeyEvent::Key {
                serial: _,
                state,
                key,
                time,
                ..
            } => {
                if let Some(xkb_state) = &mut self.xkb_state {
                    let key_direction = if state == KeyState::Pressed {
                        KeyDirection::Down
                    } else {
                        KeyDirection::Up
                    };
                    let keycode = if self.keymap_format == Some(WlKeymapFormat::XkbV1) {
                        // https://docs.rs/wayland-client/0.29.4/wayland_client/protocol/wl_keyboard/enum.KeymapFormat.html#variant.XkbV1
                        // Have to add offset to make it xkb compatible.
                        key + 8
                    } else {
                        log::error!("Undefined keymap format. Using raw keycode.");
                        key
                    };
                    let keysym = xkb_state.key_get_one_sym(keycode);
                    xkb_state.update_key(keycode, key_direction);

                    // TODO: Update the xkbcommon lib and update with `mod_names_are_active` or `serialize_mods`?
                    let key_event = CskkKeyEvent::from_keysym_with_flags(
                        keysym,
                        xkb_state.mod_name_is_active(MOD_NAME_CTRL, STATE_MODS_EFFECTIVE),
                        xkb_state.mod_name_is_active(MOD_NAME_SHIFT, STATE_MODS_EFFECTIVE),
                        xkb_state.mod_name_is_active(MOD_NAME_ALT, STATE_MODS_EFFECTIVE),
                    );
                    log::debug!("CskkKeyEvent: {:?}", key_event);
                    if state == KeyState::Pressed && self.ctx.will_process(&key_event) {
                        self.ctx.process_key_event(&key_event);
                        let mut count: u32 = 0;
                        if let Some(preedit_string) = self.ctx.get_preedit() {
                            // Set cursor to 0,0 for now
                            self.im.set_preedit_string(preedit_string, 0, 0);
                            count += 1;
                        }
                        if let Some(output_string) = self.ctx.poll_output() {
                            self.im.commit_string(output_string);
                            count += 1;
                        }
                        self.im.commit(count);
                    } else {
                        self.delegate_key(time, key, state);
                    }
                } else {
                    self.delegate_key(time, key, state);
                }
            }
            KeyEvent::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(xkb_state) = &mut self.xkb_state {
                    // Not sure of the layouts for depressed and latched...
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
                self.delegate_modifier(mods_depressed, mods_latched, mods_locked, group);
            }
            _ => {}
        }
    }

    pub fn new(
        vk: Main<ZwpVirtualKeyboardV1>,
        im: Main<ZwpInputMethodV2>,
        app_config: AppConfig,
    ) -> Result<Self, String> {
        let mut dicts = vec![Arc::new(
            CskkDictionary::new_empty_dict().expect("Create empty dictionary"),
        )];
        for user_dict in app_config.user_dictionary {
            match CskkDictionary::new_user_dict(&user_dict.path, &user_dict.encoding) {
                Ok(dict) => {
                    dicts.push(Arc::new(dict));
                }
                Err(error) => {
                    log::error!(
                        "Loading user dictionary error. Skipping {}, {:?}",
                        &user_dict.path,
                        error
                    );
                }
            }
        }
        for static_dict in app_config.static_dictionary {
            match CskkDictionary::new_static_dict(&static_dict.path, &static_dict.encoding) {
                Ok(dict) => {
                    dicts.push(Arc::new(dict));
                }
                Err(error) => {
                    log::error!(
                        "Loading static dictionary error. Skipping {}, {:?}",
                        &static_dict.path,
                        error
                    );
                }
            }
        }

        let ctx = CskkContext::new(InputMode::Ascii, CompositionMode::Direct, dicts);

        Ok(WlCskkContext {
            ctx,
            vk,
            im,
            grab: None,
            keymap_init_done: false,
            keymap_format: None,
            activating: false,
            is_active: false,
            xkb_context: Context::new(CONTEXT_NO_FLAGS),
            xkb_state: None,
        })
    }

    pub fn from_wayland_data<'a>(data: &'a mut DispatchData) -> &'a mut Self {
        data.get::<Self>().unwrap()
    }
}
