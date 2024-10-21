use std::{
    mem,
    slice,
};

use valthrun_driver_shared::{
    requests::{
        RequestKeyboardState,
        ResponseKeyboardState,
    },
    KeyboardState,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput,
    INPUT,
    INPUT_KEYBOARD,
    KEYEVENTF_KEYUP,
};

pub fn keyboard_state(
    req: &RequestKeyboardState,
    _res: &mut ResponseKeyboardState,
) -> anyhow::Result<()> {
    let states = unsafe { slice::from_raw_parts(req.buffer, req.state_count) };
    let inputs = states
        .iter()
        .map(keyboard_state_to_input)
        .collect::<Vec<_>>();

    unsafe { SendInput(&inputs, mem::size_of::<INPUT>() as i32) };
    Ok(())
}

fn keyboard_state_to_input(state: &KeyboardState) -> INPUT {
    let mut input_data: INPUT = Default::default();
    input_data.r#type = INPUT_KEYBOARD;

    let ki = unsafe { &mut input_data.Anonymous.ki };
    ki.wScan = state.scane_code;
    if !state.down {
        ki.dwFlags |= KEYEVENTF_KEYUP;
    }

    input_data
}
