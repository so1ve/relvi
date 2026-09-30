use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;

use rustix::fs::{MemfdFlags, memfd_create};
use rustix::time::{ClockId, clock_gettime};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

const KEYMAP: &str = concat!(include_str!("keymap.xkb"), "\0");
const KEY_LEFTCTRL: u32 = 29;
const KEY_V: u32 = 47;
const CONTROL: u32 = 1 << 2;

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);

pub fn paste() -> Result<(), Box<dyn Error + Send + Sync>> {
    let connection = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<State>(&connection)?;
    let handle = queue.handle();
    let manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|error| format!("Cannot bind Wayland virtual keyboard: {error}"))?;
    let seat: wl_seat::WlSeat = globals.bind(&handle, 1..=1, ())?;
    let keyboard = manager.create_virtual_keyboard(&seat, &handle, ());

    let mut keymap = File::from(memfd_create(c"relvi-keymap", MemfdFlags::CLOEXEC)?);
    keymap.write_all(KEYMAP.as_bytes())?;
    keyboard.keymap(
        KeymapFormat::XkbV1.into(),
        keymap.as_fd(),
        KEYMAP.len() as u32,
    );
    connection.roundtrip()?;

    let now = clock_gettime(ClockId::Monotonic);
    let time = (now.tv_sec * 1000 + now.tv_nsec / 1_000_000) as u32;
    keyboard.key(time, KEY_LEFTCTRL, KeyState::Pressed.into());
    keyboard.modifiers(CONTROL, 0, 0, 0);
    keyboard.key(time, KEY_V, KeyState::Pressed.into());
    keyboard.key(time, KEY_V, KeyState::Released.into());
    keyboard.key(time, KEY_LEFTCTRL, KeyState::Released.into());
    keyboard.modifiers(0, 0, 0, 0);
    connection.roundtrip()?;

    keyboard.destroy();
    connection.flush()?;

    Ok(())
}
