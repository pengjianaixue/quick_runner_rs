use clap::error::ErrorKind;
use clap::Parser;
use crossbeam_channel::unbounded;
use crossterm::event::{self, Event, KeyCode};
use serde::Serialize;
use serde_json::Result;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Error, Stdout, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::ptr::null_mut;
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::u32::MAX;
use std::{env, io, result};
use winapi::um::consoleapi::SetConsoleCtrlHandler;
use winapi::um::winbase::{CREATE_NEW_CONSOLE, DETACHED_PROCESS};
use winapi::um::wincon::CTRL_CLOSE_EVENT;
use winapi::{
    shared::minwindef::{BOOL, DWORD, FALSE, TRUE, UINT},
    um::{errhandlingapi::*, winuser::*},
};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, TRUE as Foundation_True, WPARAM};
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

#[derive(Serialize)]
struct RunContext<'a> {
    quick_cmd_name: &'a str,
    program_name: &'a str,
    work_dir: Option<&'a str>,
    create_falg: Option<u32>,
    run_type: u32,
    args: Vec<&'a str>,
    shortcut_key: u32,
}

fn run_command(run_ctxt: &RunContext) -> io::Result<Child> {
    println!("[runner]-> start run <{}>\n", run_ctxt.quick_cmd_name);
    let mut cmd_runner = Command::new(run_ctxt.program_name);
    if let Some(work_dir) = run_ctxt.work_dir {
        cmd_runner.current_dir(work_dir);
    }
    if let Some(create_falg) = run_ctxt.create_falg {
        cmd_runner.creation_flags(create_falg);
    }

    match cmd_runner
        .args(run_ctxt.args.clone())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(ret) => {
            println!("[runner]-> run cmd:<{}> success", run_ctxt.quick_cmd_name);
            Ok(ret)
        }
        Err(err) => {
            println!(
                "[runner]-> quick cmd:<{}> failed, error info:{}",
                run_ctxt.quick_cmd_name, err
            );
            Err(err)
        }
    }
}

/* system hook call back function protype */
// unsafe extern "system" fn keyboard_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
//     if code == HC_ACTION as i32 {
//         let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
//         println!("Key pressed: {}", kb_struct.scanCode);
//     }
//     CallNextHookEx(None, code, w_param, l_param)
// }
/* console handler call back function protype */
// unsafe extern "system" fn console_handler(ctrl_type: DWORD) -> BOOL {
//     if ctrl_type == CTRL_CLOSE_EVENT {
//         println!("do not close");
//         sleep(Duration::from_secs(2));
//         return TRUE;
//     }
//     FALSE
// }

/* hotkey magic number */
const HOTKEY_ID: i32 = 0xC025DE_i32;

// macro_rules! print_var {
//     ($var:ident) => {
//         println!("{} = {:?}", stringify!($var), $var);
//     };
// }
fn main() -> io::Result<()> {
    let args = Args::parse();
    for _ in 0..args.count {
        println!("Args: {}", args.json_path);
    }
    let json_path = Path::new(args.json_path.as_str());
    if !json_path.exists() {
        println!("File {} not exists", args.json_path)
    }

    let (terminal_tx, terminal_rx) = unbounded();
    let (hotkey_tx, hotkey_rx) = unbounded();
    /* add the ctrl-c input handler */
    ctrlc::set_handler(move || {
        println!("received ctrl + C");
        terminal_tx.send("exit").expect("send failed");
    })
    .expect("set handle error");

    // unsafe {
    //     if SetConsoleCtrlHandler(Some(console_handler), TRUE) == TRUE
    //     {
    //         println!("set Console Ctrl Handler success");
    //     }
    // }

    let hotkey_handler = spawn(move || {
        loop {
            let key_msg = hotkey_rx.try_recv();
            match key_msg {
                // Err(crossbeam_channel::TryRecvError::Empty) => {
                // }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    println!("ipc_msg: disconnect");
                    break;
                }
                Ok(key_code) => {
                    println!("key recv: {}", key_code);
                    if key_code == MAX as i32 {
                        println!("hot key process exit !");
                        break;
                    }
                    match key_code {
                        VK_F1 => {
                            let run_ctxt = RunContext{quick_cmd_name: "GRAPE GDB",program_name:"python.exe",work_dir:Some("D:/Code/firmware/memblaze/gdb/grape/gdb_launcher"),create_falg:Some(DETACHED_PROCESS),run_type:0,shortcut_key:VK_F1 as u32,args: vec!["D:/Code/firmware/memblaze/gdb/grape/gdb_launcher/gdb_launcher_gui_grape.py"]};
                            let _ = run_command(&run_ctxt);
                        }
                        VK_F2 => {
                            println!("start scp image to local \n");
                            let mut run_ctxt = RunContext{quick_cmd_name: "scp image to local",program_name:"scp.exe",work_dir:None,create_falg:Some(DETACHED_PROCESS),run_type:0,shortcut_key:VK_F2 as u32,args: vec!["pengjian@172.30.20.3:/home/pengjian/code/firmware/build/swap/images/*.axf","C:/Users/jian.peng/Desktop/Images/images"]
                            };
                            let _ = run_command(&run_ctxt);
                            run_ctxt.args = vec!["pengjian@172.30.20.3:/home/pengjian/code/firmware/build/swap/images/image_allbinary.bin","C:/Users/jian.peng/Desktop/Images/images"];
                            let _ = run_command(&run_ctxt);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    });
    let get_message_thread = spawn(move || unsafe {
        let hotkey_ids = vec![VK_F1, VK_F2, VK_NUMPAD2];
        let hotkey_ids_name = vec![stringify!(VK_F1), stringify!(VK_F2), stringify!(VK_NUMPAD2)];
        for hotkey_idx in 0..hotkey_ids.len() {
            println!(
                "hotkey register: {} => {}",
                hotkey_ids_name[hotkey_idx], hotkey_ids[hotkey_idx]
            );
            if RegisterHotKey(
                null_mut(),
                HOTKEY_ID,
                (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT) as UINT,
                hotkey_ids[hotkey_idx] as UINT,
            ) == 0
            {
                eprintln!("register hot key error, exit {}", GetLastError());
                return;
            }
        }
        let mut msg = MSG::default();
        loop {
            let ipc_msg = terminal_rx.try_recv();
            match ipc_msg {
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
            if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE) == Foundation_True {
                println!("rec msg");
                if msg.message == WM_HOTKEY && msg.wParam == WPARAM(HOTKEY_ID as usize) {
                    let key_pressed = msg.lParam.0 >> 16;
                    hotkey_tx.send(key_pressed as i32).unwrap();
                    println!("hot key:{:?} pressed !", key_pressed);
                }
            }
        }
        let _ = UnregisterHotKey(null_mut(), HOTKEY_ID);
    });
    /* waiting for the hook process thread */
    while !get_message_thread.is_finished() {}
    while !hotkey_handler.is_finished() {}
    Ok(())
}
