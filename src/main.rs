// Copyright (c) 2017, 2018 Marek Benc <dusxmt@gmx.com>
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
extern crate pancurses;
extern crate sdl2;
extern crate time;

use std::env;
use std::path;
use std::process;

mod cassette;
mod emulator;
mod fonts;
mod keyboard;
mod memory;
mod proj_config;
mod timing;
mod user_interface;
mod util;
mod video;
mod z80;

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

fn main() {
    user_interface::UserInterface::attach_panic_handler();

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

    let mut startup_logger = util::StartupLogger::new();

    let mut config_system = match proj_config::ConfigSystem::new(&config_dir, &mut startup_logger) {
        Some(system) => { system },
        None => {
            eprintln!("Failed to initialize the emulator.");
            user_interface::UserInterface::enter_key_to_close_on_windows();
            process::exit(1);
        }
    };
    let selected_rom;

    if rom1_selected {
        selected_rom = 1;
    } else if rom2_selected {
        selected_rom = 2;
    } else if rom3_selected {
        selected_rom = 3;
    } else {
        selected_rom = config_system.config_items.general_default_rom;
    }

    let mut memory_system = match memory::MemorySystem::new(&config_system, &mut startup_logger, selected_rom) {
        Some(system) => { system },
        None => {
            eprintln!("Failed to initialize the emulator's memory system.");
            user_interface::UserInterface::enter_key_to_close_on_windows();
            process::exit(1);
        },
    };
    let mut emulator = emulator::Emulator::new(&config_system.config_items, &mut startup_logger);
    let ask_for_enter = emulator.run(&mut memory_system, &mut config_system, startup_logger);

    if ask_for_enter {
        user_interface::UserInterface::enter_key_to_close_on_windows();
    }
    process::exit(0);
}
