// Copyright (c) 2017, 2018, 2023 Marek Benc <dusxmt@gmx.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//

extern crate getopts;
extern crate home;
extern crate pancurses;
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate sdl2;

mod cassette;
mod emulator;
mod fonts;
mod keyboard;
mod machine;
mod memory;
mod proj_config;
mod user_interface;
mod util;
mod video;
mod sdl_keyboard;
mod sdl_video;
mod z80;

use backtrace::Backtrace;
use log::{info, warn, error};

use std::sync::Mutex;
use std::sync::mpsc;
use std::vec::Vec;
use std::panic;
use std::env;
use std::path;
use std::process;
use std::thread;


lazy_static! {

    // Global panic message collection point, they're collected so that they
    // can be displayed once the curses interface is terminated.
    //
    static ref PANIC_MSGS: Mutex<Vec<String>> = Mutex::new(Vec::new());


    // Whether or not a backtrace is included in the collected panic messages.
    //
    static ref PANIC_BT: bool = match env::var("RUST_BACKTRACE") {
        Ok(text) => {
            let text_lc = text.to_lowercase();
            text_lc != "no" && text_lc != "0"
        },
        Err(..)  => { false },
    };

    // Global, thread-safe message logging mechanism.
    //
    static ref MSG_LOGGER: util::MessageLogger = util::MessageLogger::new();
}

fn print_usage(progname: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options]", progname);
    print!("{}", opts.usage(&brief));
}

// Figure out the name of the executable:
fn get_progname(arg0: &path::Path) -> String {

    match arg0.file_name() {
        Some(name_osstr) => {
            name_osstr.to_string_lossy().into_owned()
        },

        // If we can't figure it out, just guess.
        None => {
            "trs80m1-rs".to_owned()
        },
    }
}

fn entry_point() {

    // Machine control and status interface.
    //
    let (emu_cmd_tx,  emu_cmd_rx)  = mpsc::channel();
    let (emu_stat_tx, emu_stat_rx) = mpsc::channel();

    // Video control interface.
    //
    let (video_cmd_tx,  video_cmd_rx)  = mpsc::channel();
    let (video_stat_tx, video_stat_rx) = mpsc::channel();
    let emu_cmd_tx2 = emu_cmd_tx.clone();

    // Keyboard interface.
    //
    let (kbd_codes_tx, kbd_codes_rx)  = mpsc::channel();

    let args: Vec<String> = env::args().collect();
    let progname = get_progname(args[0].as_ref());

    let mut options = getopts::Options::new();
    options.optopt("c", "cfg-dir", "Override the default config directory.", "PATH");
    options.optflag("1", "", "Use the level 1 BASIC rom.");
    options.optflag("2", "", "Use the level 2 BASIC rom.");
    options.optflag("3", "", "Use the miscellaneous rom.");
    options.optflag("h", "help", "Show this help listing.");

    let matches = match options.parse(&args[1..]) {
        Ok(matches) => { matches },
        Err(error) => {
            eprintln!("{}: Argument parsing error: {}", progname, error);
            user_interface::UserInterface::enter_key_to_close_on_windows();
            process::exit(1);
        },
    };
    if matches.opt_present("h") {
        print_usage(&progname, options);
        process::exit(0);
    }
    let config_dir = match matches.opt_str("c") {
        Some(dir_path) => {
            (dir_path.as_ref() as &path::Path).to_owned()
        },
        None => {
            proj_config::get_default_config_dir_path()
        },
    };
    let rom1_selected = matches.opt_present("1");
    let rom2_selected = matches.opt_present("2");
    let rom3_selected = matches.opt_present("3");

    if (rom1_selected && rom2_selected) ||
       (rom1_selected && rom3_selected) ||
       (rom2_selected && rom3_selected) {

        eprintln!("You've specified multiple ROMs to use. Please choose only one.");
        user_interface::UserInterface::enter_key_to_close_on_windows();
        process::exit(1);
    }

    let config_system = match proj_config::ConfigSystem::new(&config_dir) {
        Some(system) => { system },
        None => {
            eprintln!("Failed to initialize the emulator.");
            user_interface::UserInterface::enter_key_to_close_on_windows();
            process::exit(1);
        }
    };

    let selected_rom = if rom1_selected {
        1
    } else if rom2_selected {
        2
    } else if rom3_selected {
        3
    } else {
        config_system.config_items.general_default_rom
    };

    info!("Switching to the curses-based user interface.");
    MSG_LOGGER.set_stdouterr_echo(false);
    let mut user_interface = match user_interface::UserInterface::new() {
        Some(user_interface) => {
            user_interface
        },
        None => {
            eprintln!("Starting the curses-based user interface failed.");
            user_interface::UserInterface::enter_key_to_close_on_windows();
            process::exit(1);
        },
    };

    thread::Builder::new().name("logic_core".to_owned()).spawn(move || {
        let mut logic_core = emulator::EmulatorLogicCore::new(emu_stat_tx, video_cmd_tx, video_stat_rx, config_system, selected_rom);
        logic_core.run(&emu_cmd_rx, &kbd_codes_rx);
    }).unwrap();

    thread::Builder::new().name("sdl2_frontend".to_owned()).spawn(move || {
        let mut sdl_frontend = emulator::EmulatorSdlFrontend::new(kbd_codes_tx, emu_cmd_tx2, video_stat_tx);
        sdl_frontend.run(&video_cmd_rx);
    }).unwrap();

    user_interface.run(&emu_cmd_tx, &emu_stat_rx, &MSG_LOGGER);
}

fn main() {
    MSG_LOGGER.set_logger().unwrap();

    let normal_panic = panic::take_hook();

    panic::set_hook(Box::new(|panic_info| {
        let thread_name = match thread::current().name() {
            Some(name) => { name.to_owned() },
            None => { "<unnamed>".to_owned() },
        };
        let panic_prefix = format!("thread '{}' panicked at '", thread_name);

        let panic_suffix = match panic_info.location() {
            Some(location) => {
                format!("', {}:{}:{}", location.file(), location.line(), location.column())
            },
            None => {
                "', unable to determine the location.".to_owned()
            },
        };
        let panic_message = match panic_info.payload().downcast_ref::<&'static str>() {
            Some(string_message) => string_message,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(string_message) => string_message.as_str(),
                None => "Unknown",
            },
        };
        let bt_message = if *PANIC_BT {
            let bt = Backtrace::new();
            format!("stack backtrace:\n{:?}", bt)
        } else {
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace".to_owned()
        };

        match PANIC_MSGS.lock() {
            Ok(mut err_vec) => {
                err_vec.push(format!("{}{}{}", panic_prefix, panic_message, panic_suffix));
                err_vec.push(bt_message);
            },
            Err(error) => {
                eprintln!("Locking the global panic message buffer failed ({}), printing panic info to stderr:", error);
                eprintln!("{}{}{}", panic_prefix, panic_message, panic_suffix);
                eprintln!("{}", bt_message);
            }
        }
    }));

    // The return value here is ignored, because we know whether or not a panic
    // occured based on the panic log.
    //
    // This return value only represents panics that took place in the
    // main thread, not in the other threads, thus it's less useful than
    // the more general panic log maintained by the custom panic handler.
    //
    let _ = panic::catch_unwind(|| {
        entry_point();
    });
    panic::set_hook(Box::new(normal_panic));

    let mut found_err = false;
    let err_vec = PANIC_MSGS.lock().unwrap();
    for msg in &*err_vec {
        eprintln!("{}", msg);
        found_err = true;
    }

    // Potential race condition: If a panic in a separate thread takes place
    // after the normal panic handler is restored, we will not be aware of it,
    // and the program will report a "success" exit code.
    //
    // Mitigation: Do not leave the catch_unwind() block gracefully unless all
    // child threads have been terminated.
    //
    if found_err {
        user_interface::UserInterface::enter_key_to_close_on_windows();
        process::exit(101);
    }
}
