use std::sync::mpsc::Sender;

use evdev;
use xkbcommon::xkb;

const KEYCODE_OFFSET: u16 = 8;

enum KeyState {
    Release,
    Press,
    Repeat,
}

impl TryFrom<i32> for KeyState {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == KeyState::Release as i32 => Ok(KeyState::Release),
            x if x == KeyState::Press as i32 => Ok(KeyState::Press),
            x if x == KeyState::Repeat as i32 => Ok(KeyState::Repeat),
            _ => Err(()),
        }
    }
}

pub fn read_input(device_path: &str, tx: Sender<String>) {
    // Open evdev device
    let mut device = evdev::Device::open(device_path).expect("Could not open device");
    device.grab().expect("Could not exclusively grab device");

    // Create context
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

    // Load keymap informations
    let keymap = xkb::Keymap::new_from_names(
        &context,
        "",      // rules
        "pc105", // model
        "us",    // layout
        "",      // variant
        None,    // options
        xkb::COMPILE_NO_FLAGS,
    )
    .unwrap();

    // Create the state tracker
    let mut state = xkb::State::new(&keymap);
    let mut linebuf = String::with_capacity(50);
    loop {
        for event in device.fetch_events().unwrap() {
            if let evdev::EventSummary::Key(_, ev_keycode, dir) = event.destructure() {
                let keystate = KeyState::try_from(dir).expect("Invalid keystate");
                let xkb_keycode = (ev_keycode.0 + KEYCODE_OFFSET).into();
                match keystate {
                    KeyState::Repeat => {
                        continue;
                    }
                    KeyState::Release => {
                        state.update_key(xkb_keycode, xkb::KeyDirection::Up);
                    }
                    KeyState::Press => {
                        state.update_key(xkb_keycode, xkb::KeyDirection::Down);
                        let key = state.key_get_utf8(xkb_keycode);
                        if ev_keycode == evdev::KeyCode::KEY_ENTER {
                            if !linebuf.is_empty() {
                                tx.send(linebuf.clone()).unwrap();
                                linebuf.clear();
                            }
                        } else if !key.is_empty() {
                            linebuf.push_str(&key);
                        }
                    }
                }
            }
        }
    }
}
