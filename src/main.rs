use clap::Parser;
use crossbeam_channel::unbounded;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::from_str;
use std::collections::HashMap;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::ptr::null_mut;
use std::str::FromStr;
use std::thread::spawn;
use std::u32::MAX;
use std::{env, io};
use winapi::um::winbase::{CREATE_NEW_CONSOLE, DETACHED_PROCESS};
use winapi::{
    shared::minwindef::UINT,
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

#[derive(Deserialize, Debug)]
struct Cmd_Context<'a> {
    quick_cmd_name: &'a str,
    run_type: Option<&'a str>,
    program_name: &'a str,
    process_create_attr: &'a str,
    work_dir: Option<&'a str>,
    shortcut_key_name: &'a str,
    #[serde(skip)]
    shortcut_key_code: Option<u32>,
    args: Vec<&'a str>,
}

fn get_process_attr_from_name(process_create_flag: &str) -> Option<u32> {
    match process_create_flag {
        "CREATE_NEW_CONSOLE" => Some(DETACHED_PROCESS),
        "DETACHED_PROCESS" => Some(DETACHED_PROCESS),
        _ => None,
    }
}
fn run_command(run_ctxt: &Cmd_Context) -> io::Result<Child> {
    println!("[runner]-> start run <{}>\n", run_ctxt.quick_cmd_name);
    let mut cmd_runner = Command::new(run_ctxt.program_name);
    if let Some(work_dir) = run_ctxt.work_dir {
        cmd_runner.current_dir(work_dir);
    }

    if let Some(create_flag) = get_process_attr_from_name(run_ctxt.process_create_attr) {
        cmd_runner.creation_flags(create_flag);
    } else {
        println!(
            "[runner]-> run cmd:<{}> failed, unknow create flag:{}",
            run_ctxt.quick_cmd_name, run_ctxt.process_create_attr
        );
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

fn create_virtual_key_map() -> HashMap<&'static str, u32> {
    let mut key_map = HashMap::new();

    // key_map.insert("VK_LBUTTON", 0x01);
    // key_map.insert("VK_RBUTTON", 0x02);
    // key_map.insert("VK_CANCEL", 0x03);
    // key_map.insert("VK_MBUTTON", 0x04);
    // key_map.insert("VK_XBUTTON1", 0x05);
    // key_map.insert("VK_XBUTTON2", 0x06);
    // key_map.insert("VK_BACK", 0x08);
    // key_map.insert("VK_TAB", 0x09);
    // key_map.insert("VK_RETURN", 0x0D);
    // key_map.insert("VK_SHIFT", 0x10);
    // key_map.insert("VK_CONTROL", 0x11);
    // key_map.insert("VK_MENU", 0x12);
    // key_map.insert("VK_PAUSE", 0x13);
    // key_map.insert("VK_CAPITAL", 0x14);
    // key_map.insert("VK_ESCAPE", 0x1B);
    // key_map.insert("VK_SPACE", 0x20);
    key_map.insert("VK_NUMPAD0", 0x60);
    key_map.insert("VK_NUMPAD1", 0x61);
    key_map.insert("VK_NUMPAD2", 0x62);
    key_map.insert("VK_F1", 0x70);
    key_map.insert("VK_F2", 0x71);
    key_map.insert("VK_F3", 0x72);
    key_map
}

fn get_virtual_key_code(key_name: &str) -> Option<u32> {
    let key_map = create_virtual_key_map();
    key_map.get(key_name).copied()
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
static mut CONTENTS: std::string::String = String::new();

// macro_rules! print_var {
//     ($var:ident) => {
//         println!("{} = {:?}", stringify!($var), $var);
//     };
// }
static mut CMD_CONFIG_MAP: Lazy<HashMap<u32, Cmd_Context<'static>>> = Lazy::new(|| {
    let m = HashMap::new();
    m
});

fn main() -> io::Result<()> {
    let args = Args::parse();
    for _ in 0..args.count {
        println!("Args: {}", args.json_path);
    }
    let json_path = Path::new(args.json_path.as_str());
    if !json_path.exists() {
        println!("File {} not exists", args.json_path)
    }
    // let mut cmd_config_map: HashMap<u32, Cmd_Context> = HashMap::new();
    let mut file = std::fs::File::open(json_path).unwrap();

    file.read_to_string(unsafe { &mut CONTENTS })?;

    let cmd_config_items: Vec<Cmd_Context> = from_str(unsafe { CONTENTS.as_str() })?;
    for mut cmd_item in cmd_config_items {
        println!("config_cmd : {:?}", cmd_item);
        cmd_item.shortcut_key_code = get_virtual_key_code(cmd_item.shortcut_key_name);
        if let Some(shortcut_key) = cmd_item.shortcut_key_code {
            if unsafe { CMD_CONFIG_MAP.contains_key(&shortcut_key) } {
                println!(
                    "the cmd: [{}] shortcut key [{}] conflict",
                    cmd_item.quick_cmd_name, cmd_item.shortcut_key_name
                );
            } else {
                unsafe { CMD_CONFIG_MAP.insert(shortcut_key, cmd_item) };
            }
        } else {
            println!(
                "[Err] => cmd:[{}] shortcut key[{}] do not support",
                cmd_item.quick_cmd_name, cmd_item.shortcut_key_name
            );
        }
    }
    let (terminal_tx, terminal_rx) = unbounded();
    let (hotkey_tx, hotkey_rx) = unbounded();
    /* add the ctrl-c input handler */
    ctrlc::set_handler(move || {
        println!("received ctrl + C");
        terminal_tx
            .send(String::from_str("exit").unwrap())
            .expect("send failed");
    })
    .expect("set handle error");

    // unsafe {
    //     if SetConsoleCtrlHandler(Some(console_handler), TRUE) == TRUE
    //     {
    //         println!("set Console Ctrl Handler success");
    //     }
    // }

    let hotkey_handler = hotkey_handler(unsafe { &CMD_CONFIG_MAP }, hotkey_rx);
    let get_message_thread =
        hotkey_register_and_monitor(unsafe { &CMD_CONFIG_MAP }, terminal_rx, hotkey_tx);
    /* waiting for the hook process thread */
    while !get_message_thread.is_finished() {}
    while !hotkey_handler.is_finished() {}
    Ok(())
}

fn hotkey_handler(
    cmd_config_map: &'static HashMap<u32, Cmd_Context>,
    hotkey_rx: crossbeam_channel::Receiver<i32>,
) -> std::thread::JoinHandle<()> {
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
                    if let Some(cmd_ctxt) = cmd_config_map.get(&(key_code as u32)) {
                        let _ = run_command(&cmd_ctxt);
                    }
                }
                _ => {}
            }
        }
    });
    hotkey_handler
}

fn hotkey_register_and_monitor(
    cmd_config_map: &'static HashMap<u32, Cmd_Context>,
    terminal_rx: crossbeam_channel::Receiver<String>,
    hotkey_tx: crossbeam_channel::Sender<i32>,
) -> std::thread::JoinHandle<()> {
    let get_message_thread = spawn(move || unsafe {
        for (hotkey, cmd_context) in cmd_config_map {
            if RegisterHotKey(
                null_mut(),
                HOTKEY_ID,
                (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT) as UINT,
                (*hotkey) as UINT,
            ) == 0
            {
                eprintln!("register hot key error, exit {}", GetLastError());
                return;
            } else {
                println!(
                    "hotkey register: {} => {}",
                    cmd_context.shortcut_key_name,
                    cmd_context.shortcut_key_code.unwrap()
                );
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
    get_message_thread
}
