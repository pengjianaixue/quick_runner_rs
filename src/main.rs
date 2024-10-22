use clap::Parser;
use crossbeam_channel::unbounded;
use crossterm::event::{self, Event, KeyCode};
use serde_json::Result;
use std::io::Write;
use std::path::Path;
use std::ptr::null_mut;
use std::thread::spawn;
use std::{env, io};

use winapi::{
    shared::minwindef::UINT,
    um::{errhandlingapi::*, winuser::*},
};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, TRUE, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PeekMessageA, SetWindowsHookExA, UnhookWindowsHookEx, HC_ACTION,
    KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, PM_REMOVE, WH_KEYBOARD_LL,
};
/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long, default_value_t = String::from(env::current_dir().unwrap().to_str().unwrap()) + "\\config.json")]
    json_path: String,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

unsafe extern "system" fn keyboard_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        println!("Key pressed: {}", kb_struct.scanCode);
    }
    CallNextHookEx(None, code, w_param, l_param)
}
/* hotkey magic number */
const HOTKEY_ID: i32 = 0xC025DE_i32;

fn main() -> io::Result<()> {
    let args = Args::parse();
    for _ in 0..args.count {
        println!("Args: {}", args.json_path);
    }
    let json_path = Path::new(args.json_path.as_str());
    if !json_path.exists() {
        println!("File {} not exists", args.json_path)
    }

    let (tx, rx) = unbounded();
    // let tx_copy = tx.clone();
    /* add the ctrl-c input handler */
    ctrlc::set_handler(move || {
        println!("received ctrl + C");
        tx.send("exit").expect("send failed");
    })
    .expect("set handle error");

    let get_message_thread = spawn(move || unsafe {
        if RegisterHotKey(
            null_mut(),
            HOTKEY_ID,
            (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT) as UINT,
            VK_NUMPAD0 as UINT,
        ) == 0
        {
            eprintln!("register hot key error, exit {}", GetLastError());
            return;
        }
        let mut msg = MSG::default();
        loop {
            let ipc_msg = rx.try_recv();
            match ipc_msg {
                // Err(crossbeam_channel::TryRecvError::Empty) => {
                // }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    println!("ipc_msg: disconnect");
                    break;
                }
                Ok(msg_ipc) => {
                    println!("msg: {}", msg_ipc);
                    if msg_ipc == "exit" {
                        println!("exit !");
                        break;
                    }
                }
                _ => {}
            }
            if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE) == TRUE {
                println!("rec msg");
                if msg.message == WM_HOTKEY && msg.wParam == WPARAM(HOTKEY_ID as usize) {
                    let key_pressed = msg.lParam.0 >> 16;
                    println!("hot key:{:?} pressed !", key_pressed);
                }
            }
        }
        let _ = UnregisterHotKey(null_mut(), HOTKEY_ID);
    });
    /* waiting for the hook process thread */
    while !get_message_thread.is_finished() {}
    Ok(())
}
