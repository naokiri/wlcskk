mod wl_cskk_context;

use clap::Parser;
use cskk::CskkContext;
use log;
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_mio::v0_8::Signals;
use std::io::ErrorKind;
use std::time::Duration;
use wayland_client::{
    protocol::{wl_keyboard::KeyState, wl_seat::WlSeat},
    DispatchData, Display, Filter, GlobalManager, Main,
};
use wayland_protocols::misc::zwp_input_method_v2;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::Event as KeyEvent;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::Event as IMEvent;
use wayland_protocols::misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use wl_cskk_context::WlCskkContext;
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

/// wayland input protocol based skk input method
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;

    let _args = Args::parse();

    let display = Display::connect_to_env().expect("Failed to connect wayland display");
    let mut event_queue = display.create_event_queue();
    let attached_display = display.attach(event_queue.token());
    let globals = GlobalManager::new(&attached_display);

    event_queue.sync_roundtrip(&mut (), |_, _, _| ()).unwrap();

    // https://wayland-book.com/seat.html The input devices
    // https://wayland.app/protocols/wayland#wl_seat
    let seat: Main<WlSeat> = globals.instantiate_exact::<WlSeat>(7).expect("Load Seat");

    // Virtual keyboard is an external protocol as of now. Ridiculous.
    // https://docs.rs/zwp-virtual-keyboard/latest/zwp_virtual_keyboard/virtual_keyboard_unstable_v1/index.html
    let vk_manager = globals
        .instantiate_exact::<ZwpVirtualKeyboardManagerV1>(1)
        .expect("Load VirtualKeyboardManager");
    let vk = vk_manager.create_virtual_keyboard(&seat);

    // https://docs.rs/wayland-protocols/latest/wayland_protocols/misc/zwp_input_method_v2/client/zwp_input_method_manager_v2/struct.ZwpInputMethodManagerV2.html
    let im_manager: Main<ZwpInputMethodManagerV2> = globals
        .instantiate_exact::<ZwpInputMethodManagerV2>(1)
        .expect("Load InputManager");
    let im = im_manager.get_input_method(&seat);
    log::debug!("Grabbing keyboard");
    let grab = im.grab_keyboard();

    // Handle events using the filters
    grab.quick_assign(
        |handle: Main<ZwpInputMethodKeyboardGrabV2>, ev: KeyEvent, mut data: DispatchData| {
            let context = WlCskkContext::from_wayland_data(&mut data);
            context.handle_key_ev(ev)
        },
    );
    im.quick_assign(
        |handle: Main<ZwpInputMethodV2>, ev: IMEvent, mut data: DispatchData| {
            let context = WlCskkContext::from_wayland_data(&mut data);
            context.handle_im_ev(ev)
        },
    );

    let mut context = WlCskkContext::new(vk, im);
    if let Err(e) =
    event_queue.sync_roundtrip(&mut context, |_, _, _| log::debug!("Unhandled event"))
    {
        if let Some(protocol_error) = &display.protocol_error() {
            log::error!("Error on use of wayland protocol: {:?}", protocol_error);
        }
        return Err(Box::new(e));
    }

    let mut poll = Poll::new()?;
    let registry = poll.registry();
    const WAYLAND_FD: Token = Token(0);
    // https://wayland-book.com/wayland-display/event-loop.html
    registry
        .register(
            &mut SourceFd(&display.get_connection_fd()),
            WAYLAND_FD,
            Interest::READABLE | Interest::WRITABLE,
        )
        .expect("Registering wayland socket to poll");

    const SIGNAL: Token = Token(1);
    poll.registry()
        .register(&mut signals, SIGNAL, Interest::READABLE)?;

    log::info!("Setup done, run main loop");

    // Don't rely on event_queue's non-blocking functions and using polling as the source of event.
    let mut events = Events::with_capacity(1024);
    let stopped = 'main: loop {
        if let Err(e) = poll.poll(&mut events, None) {
            if e.kind() == ErrorKind::Interrupted {
                continue;
            }
            break Err(e);
        }

        for event in events.iter() {
            match event.token() {
                WAYLAND_FD => {
                    if let Some(guard) = event_queue.prepare_read() {
                        if let Err(e) = guard.read_events() {
                            if e.kind() != ErrorKind::WouldBlock {
                                break 'main Err(e);
                            }
                        }
                    }

                    if let Err(e) = event_queue
                        .dispatch_pending(&mut context, |_, _, _| log::debug!("Unhandled event"))
                    {
                        if let Some(protocol_error) = &display.protocol_error() {
                            log::error!("Error on use of wayland protocol: {:?}", protocol_error);
                        }
                        break 'main Err(e);
                    }

                    if let Err(e) = display.flush() {
                        if e.kind() != ErrorKind::WouldBlock {
                            log::error!("Error while trying to flush the wayland socket: {:?}", e);
                            break 'main Err(e);
                        }
                    }
                }
                SIGNAL => {
                    for signal in signals.pending() {
                        match signal {
                            SIGINT => break 'main Err("SIGINT received")?,
                            SIGTERM => break 'main Err("SIGTERM received")?,
                            _ => unreachable!(),
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    };

    match stopped {
        Err(e) => {
            log::error!("Server stopped due to error: {}", e);
            Err(Box::new(e))
        }
        Ok(()) => {
            log::info!("Server stopped gracefully");
            Ok(())
        }
    }
}
