use clap::Parser;
use crossbeam_channel::{unbounded, Receiver, Sender};
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
use std::sync::{Arc, RwLock};
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::u32::MAX;
use std::{env, io};
use winapi::um::winbase::{CREATE_NEW_CONSOLE, DETACHED_PROCESS};
use winapi::{
    shared::minwindef::UINT,
    um::{errhandlingapi::*, winuser::*},
};
// use winapi::shared::windef::HWND;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::{
    HINSTANCE, LPARAM, LRESULT, TRUE as Foundation_True, TRUE, WPARAM,
};
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
    cmd_config_json_path: String,

    #[arg(short, long, default_value_t = String::from(env::current_dir().unwrap().to_str().unwrap()) + "\\virtual_keys_codes.json")]
    key_code_json_path: String,
    /// Number of times to greet
    #[arg(short, long, default_value_t = 2)]
    arg_count: u8,
}

#[derive(Deserialize, Debug)]
struct Key_Code<'a> {
    key: &'a str,
    code: &'a str,
    comment: &'a str,
}

#[derive(Deserialize, Debug)]
struct CmdContext<'a> {
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
        "CREATE_NEW_CONSOLE" => Some(CREATE_NEW_CONSOLE),
        "DETACHED_PROCESS" => Some(DETACHED_PROCESS),
        _ => None,
    }
}
fn run_command(run_ctxt: &CmdContext) -> io::Result<Child> {
    println!("[runner]-> start run <{}>\n", run_ctxt.quick_cmd_name);
    let mut cmd_runner = Command::new(run_ctxt.program_name);
    if let Some(work_dir) = run_ctxt.work_dir {
        cmd_runner.current_dir(work_dir);
    }

    if let Some(create_flag) = get_process_attr_from_name(run_ctxt.process_create_attr) {
        if create_flag != CREATE_NEW_CONSOLE {
            cmd_runner.stdout(Stdio::piped());
        }
        cmd_runner.creation_flags(create_flag);
    } else {
        println!(
            "[runner]-> run cmd:<{}> failed, unknow create flag:{}",
            run_ctxt.quick_cmd_name, run_ctxt.process_create_attr
        );
    }

    match cmd_runner.args(run_ctxt.args.clone()).spawn() {
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

fn create_virtual_key_map(
    key_map: &mut HashMap<&str, u32>,
    key_json_file_path: &str,
) -> Option<u32> {
    let key_contents = Box::new(String::new());
    let key_contents_ref = Box::leak(key_contents);
    let mut key_config_file = std::fs::File::open(key_json_file_path).unwrap();

    key_config_file.read_to_string(key_contents_ref).unwrap();
    let key_config_items: Vec<crate::Key_Code> = from_str(key_contents_ref.as_str()).unwrap();
    for mut cmd_item in key_config_items {
        key_map.insert(
            cmd_item.key,
            u32::from_str_radix(cmd_item.code.trim_start_matches("0x"), 16).unwrap(),
        );
    }
    Some(0)
}

fn create_cmd_config(
    cmd_json_path: &'static str,
    virtual_key_map_ref: &'static HashMap<&str, u32>,
    cmd_config_map_ref: &Arc<RwLock<HashMap<u32, CmdContext>>>,
    is_reload:bool,
) -> io::Result<()> {
    let contents = Box::new(String::new());
    let contents_ref = Box::leak(contents);
    let mut cmd_config_file = std::fs::File::open(cmd_json_path).unwrap();
    cmd_config_file.read_to_string(contents_ref)?;
    let cmd_config_items: Vec<CmdContext> = from_str(contents_ref.as_str())?;
    if is_reload {
        cmd_config_map_ref.write().unwrap().clear();
    }
    for mut cmd_item in cmd_config_items {
        println!("config_cmd : {:?}", cmd_item);
        cmd_item.shortcut_key_code =
            get_virtual_key_code(virtual_key_map_ref, cmd_item.shortcut_key_name);
        if let Some(shortcut_key) = cmd_item.shortcut_key_code {
            if cmd_config_map_ref
                .read()
                .unwrap()
                .contains_key(&shortcut_key)
            {
                println!(
                    "the cmd: [{}] shortcut key [{}] conflict",
                    cmd_item.quick_cmd_name, cmd_item.shortcut_key_name
                );
            } else {
                cmd_config_map_ref
                    .write()
                    .unwrap()
                    .insert(shortcut_key, cmd_item);
            }
        } else {
            println!(
                "[Err] => cmd:[{}] shortcut key[{}] do not support",
                cmd_item.quick_cmd_name, cmd_item.shortcut_key_name
            );
        }
    }
    io::Result::Ok(())
}

fn get_virtual_key_code(key_map: &HashMap<&str, u32>, key_name: &str) -> Option<u32> {
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

// static mut CONTENTS: std::string::String = String::new();

// macro_rules! print_var {
//     ($var:ident) => {
//         println!("{} = {:?}", stringify!($var), $var);
//     };
// }

// static mut CMD_CONFIG_MAP: Lazy<HashMap<u32, CmdContext<'static>>> = Lazy::new(|| {
//     let m = HashMap::new();
//     m
// });

fn main() -> io::Result<()> {
    let args = Args::parse();
    for _ in 0..args.arg_count {
        println!("Args: {}", args.cmd_config_json_path);
    }
    let json_path = Path::new(args.cmd_config_json_path.as_str());
    if !json_path.exists() {
        println!("File {} not exists", args.cmd_config_json_path)
    }
    let cmd_config_json_path = Box::leak(Box::new(args.cmd_config_json_path).into_boxed_str());
    let mut virtual_key_map: Box<HashMap<&str, u32>> = Box::new(HashMap::new());
    let virtual_key_map_ref: &'static mut HashMap<&str, u32> = Box::leak(virtual_key_map);
    create_virtual_key_map(virtual_key_map_ref, &args.key_code_json_path).unwrap();
    let cmd_config_map = Box::new(Arc::new(RwLock::new(HashMap::new())));
    // let  cmd_config_map: &'static mut Arc<RwLock<HashMap<u32, CmdContext>>>=  Box::leak(Arc::new(RwLock::new(HashMap::new())));
    let cmd_config_map_mut_ref: &'static mut Arc<RwLock<HashMap<u32, CmdContext>>> =
        Box::leak(cmd_config_map);
    create_cmd_config(
        cmd_config_json_path,
        virtual_key_map_ref,
        &cmd_config_map_mut_ref.clone(),
        false
    )
    .expect("parse command config failed");
    let (terminal_tx, terminal_rx) = unbounded();
    let (hotkey_tx, hotkey_rx) = unbounded();
    let (config_change_tx, config_change_rx) = unbounded();
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

    let hotkey_handler = hotkey_handler(
        cmd_config_json_path,
        virtual_key_map_ref,
        cmd_config_map_mut_ref,
        hotkey_rx,
        config_change_tx,
    );
    let get_message_thread = hotkey_register_and_monitor(
        cmd_config_map_mut_ref,
        terminal_rx,
        hotkey_tx,
        config_change_rx,
    );
    /* waiting for the hook process thread */
    get_message_thread.join().unwrap();
    hotkey_handler.join().unwrap();
    println!("main process exit");
    Ok(())
}

fn hotkey_handler(
    cmd_file_path: &'static str,
    vf_keymap: &'static HashMap<&str, u32>,
    mut cmd_config_map: &'static Arc<RwLock<HashMap<u32, CmdContext>>>,
    hotkey_rx: crossbeam_channel::Receiver<i32>,
    config_change_tx: Sender<u32>,
) -> std::thread::JoinHandle<()> {
    let hotkey_handler = spawn(move || loop {
        let cmd_config_map_local = cmd_config_map.clone();
        let key_msg = hotkey_rx.try_recv();
        match key_msg {
            Err(crossbeam_channel::TryRecvError::Empty) => {
                sleep(Duration::from_millis(1));
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                println!("ipc_msg: disconnect");
                break;
            }
            Ok(key_code) => {
                println!("key recv: {}", key_code);
                if key_code == MAX as i32 {
                    println!("hot key process exit !");
                    break;
                } else if key_code == (*(vf_keymap.get("VK_F9").unwrap()) as i32) {
                    println!("re-load cmd config !");
                    create_cmd_config(cmd_file_path, &vf_keymap, &cmd_config_map_local,true)
                        .expect("reload cmd config fail");
                    config_change_tx.send(1).expect("send failed");
                }
                if let Some(cmd_ctxt) = cmd_config_map_local.read().unwrap().get(&(key_code as u32))
                {
                    let _ = run_command(&cmd_ctxt);
                }
            }
        }
    });
    hotkey_handler
}

fn hotkey_register_and_monitor(
    cmd_config_map: &'static Arc<RwLock<HashMap<u32, CmdContext>>>,
    terminal_rx: crossbeam_channel::Receiver<String>,
    hotkey_tx: crossbeam_channel::Sender<i32>,
    config_change_rx: Receiver<u32>,
) -> std::thread::JoinHandle<()> {
    let get_message_thread = spawn(move || unsafe {
        let mut hwnd: winapi::shared::windef::HWND = std::mem::zeroed();
        if !register_cmd_hot_key(cmd_config_map) {
            return;
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
            let config_change_notify = config_change_rx.try_recv();
            match config_change_notify {
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    println!("config_change: disconnect");
                    break;
                }
                Ok(config_change_notify) => {
                    println!("config re-load msg: {}", config_change_notify);
                    if config_change_notify == 1 {
                        println!("config reload process ack !");
                        if !register_cmd_hot_key(cmd_config_map) {
                            println!("register_cmd_hot_key exit !");
                            return;
                        }
                    }
                }
                _ => {}
            }
            if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE) == Foundation_True {
                println!("rec msg");
                if msg.message == WM_HOTKEY && msg.wParam == WPARAM(HOTKEY_ID as usize) {
                    let key_pressed = msg.lParam.0 >> 16;
                    if key_pressed == 0x78 {
                        println!("0x78 pressed");
                        if UnregisterHotKey(null_mut(), HOTKEY_ID) != 1
                        {
                            println!("UnregisterHotKey failed");
                            return;
                        }
                    }
                    hotkey_tx.send(key_pressed as i32).unwrap();
                    println!("hot key:{:?} pressed !", key_pressed);
                }
            } else {
                sleep(Duration::from_millis(1));
            }
        }
        let _ = UnregisterHotKey(null_mut(), HOTKEY_ID);
    });
    get_message_thread
}

fn register_cmd_hot_key(cmd_config_map: &Arc<RwLock<HashMap<u32, CmdContext>>>) -> bool {
    unsafe {
        if RegisterHotKey(
            null_mut(),
            HOTKEY_ID,
            (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT) as UINT,
            (0x78) as UINT,
        ) == 0
        {
            eprintln!("register hot key error, exit {}", GetLastError());
            return false;
        } else {
            println!("reload hot key register success");
        }
        for (hotkey, cmd_context) in cmd_config_map.read().unwrap().iter() {
            if RegisterHotKey(
                null_mut(),
                HOTKEY_ID,
                (MOD_ALT | MOD_CONTROL | MOD_NOREPEAT) as UINT,
                (*hotkey) as UINT,
            ) == 0
            {
                eprintln!("register hot key error, exit {}", GetLastError());
                return false;
            } else {
                println!(
                    "hotkey register: {} => {}",
                    cmd_context.shortcut_key_name,
                    cmd_context.shortcut_key_code.unwrap()
                );
            }
        }
        true
    }
}
