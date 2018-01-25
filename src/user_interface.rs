// Copyright (c) 2018 Marek Benc <dusxmt@gmx.com>
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

use std::collections::VecDeque;
use std::io;
use std::io::Read;
use std::io::Write;
use std::panic;

use emulator;
use memory;
use memory::MemoryChipOps;
use pancurses;
use util;
use util::MessageLogging;

// Program name and version:
const PROGRAM_NAME:    &str = env!("CARGO_PKG_NAME");
const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");

// Constants defining properties of the interface:
const MIN_SCREEN_WIDTH:            usize = 45;
const MIN_SCREEN_HEIGHT:           usize = 10;

const LINES_TOP_OFFSET:            usize = 1;
const LINES_BOTTOM_OFFSET:         usize = 2;

const PROMPT_BOTTOM_OFFSET:        usize = 0;
const PROMPT_TEXT_OFFSET:          usize = 2;

const TOP_STRIP_TOP_OFFSET:        usize = 0;
const BOTTOM_STRIP_BOTTOM_OFFSET:  usize = 1;

const MAX_SCREEN_LINES:            usize = 5000;
const MAX_HISTORY_ENTRIES:         usize = 500;


// Possible color pairs:
const COLOR_PAIR_STRIP_GRAY:  u8 = 1;
const COLOR_PAIR_STRIP_RED:   u8 = 2;
const COLOR_PAIR_STRIP_GREEN: u8 = 3;
const COLOR_PAIR_STRIP_CYAN:  u8 = 4;
const COLOR_PAIR_EMSG:        u8 = 5;
const COLOR_PAIR_MMSG:        u8 = 6;
const COLOR_PAIR_PROMPT:      u8 = 7;

#[derive(Clone, PartialEq)]
pub enum ScreenLineType {
    EmulatorMessage,
    MachineMessage { complete: bool },
}
struct ScreenLine {
    line_type:    ScreenLineType,
    line_content: String,
}

impl ScreenLine {
    fn physical_lines(&self, screen_width: usize) -> usize {
        let length = self.line_content.len();

        (length / screen_width) + if (length % screen_width) != 0 { 1 } else if length == 0 { 1 } else { 0 }
    }
    fn last_physical_line_len(&self, screen_width: usize) -> usize {
        let length = self.line_content.len();

        length % screen_width
    }
}

pub struct UserInterface {
    window:                      pancurses::Window,

    screen_width:                usize,
    screen_height:               usize,
    screen_too_small:            bool,

    redraw_needed:               bool,

    max_screen_lines:            usize,
    screen_lines:                VecDeque<ScreenLine>,
    lines_scroll:                usize,
    lines_added_scrolled_up:     bool,
    emulator_msg_shown:          bool,
    machine_msg_shown:           bool,

    prompt_text:                 String,
    prompt_scroll:               usize,
    prompt_curs_pos:             usize,  // relative to the prompt text

    prompt_history:              VecDeque<String>,
    prompt_history_max_entries:  usize,
    prompt_history_pos:          usize,  // 0 refers to prompt_text

    status_displayed_halted:     bool,
}

impl UserInterface {
    pub fn new() -> Option<UserInterface> {

        let window = pancurses::initscr();
        pancurses::start_color();
        pancurses::cbreak();
        pancurses::noecho();
        pancurses::nonl();
        window.nodelay(true);
        window.keypad(true);

        pancurses::init_pair(COLOR_PAIR_STRIP_GRAY   as i16,  pancurses::COLOR_WHITE,  pancurses::COLOR_BLUE);
        pancurses::init_pair(COLOR_PAIR_STRIP_RED    as i16,  pancurses::COLOR_RED,    pancurses::COLOR_BLUE);
        pancurses::init_pair(COLOR_PAIR_STRIP_GREEN  as i16,  pancurses::COLOR_GREEN,  pancurses::COLOR_BLUE);
        pancurses::init_pair(COLOR_PAIR_STRIP_CYAN   as i16,  pancurses::COLOR_CYAN,   pancurses::COLOR_BLUE);
        pancurses::init_pair(COLOR_PAIR_EMSG         as i16,  pancurses::COLOR_YELLOW, pancurses::COLOR_BLACK);
        pancurses::init_pair(COLOR_PAIR_MMSG         as i16,  pancurses::COLOR_WHITE,  pancurses::COLOR_BLACK);
        pancurses::init_pair(COLOR_PAIR_PROMPT       as i16,  pancurses::COLOR_WHITE,  pancurses::COLOR_BLACK);

        let mut user_interface = UserInterface {
                                     window:                      window,

                                     screen_width:                0,
                                     screen_height:               0,
                                     screen_too_small:            true,

                                     redraw_needed:               true,

                                     max_screen_lines:            MAX_SCREEN_LINES,
                                     screen_lines:                VecDeque::with_capacity(MAX_SCREEN_LINES),
                                     lines_scroll:                0,
                                     lines_added_scrolled_up:     false,
                                     emulator_msg_shown:          true,
                                     machine_msg_shown:           true,

                                     prompt_text:                 "".to_owned(),
                                     prompt_scroll:               0,
                                     prompt_curs_pos:             0,

                                     prompt_history:              VecDeque::with_capacity(MAX_HISTORY_ENTRIES),
                                     prompt_history_max_entries:  MAX_HISTORY_ENTRIES,
                                     prompt_history_pos:          0,

                                     status_displayed_halted:     true,
                                 };
        user_interface.handle_resize_event();

        Some(user_interface)
    }
    pub fn consume_startup_logger(&mut self, mut startup_logger: util::StartupLogger) {
        for message in startup_logger.collect_messages() {
            self.emulator_message(message.as_str());
        }
    }
    pub fn handle_user_input(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        loop {
            let user_input = self.window.getch();
            match user_input {
                Some(input) => {
                    match input {
                        pancurses::Input::KeyResize     => { self.handle_resize_event() },

                        pancurses::Input::KeyF1         => { self.execute_command("help", emulator, memory_system); },

                        pancurses::Input::KeyNPage      => { self.scroll_lines_down(); },
                        pancurses::Input::KeyPPage      => { self.scroll_lines_up(); },

                        pancurses::Input::KeyLeft       => { self.prompt_move_cursor_left(); },
                        pancurses::Input::KeyRight      => { self.prompt_move_cursor_right(); },
                        pancurses::Input::KeyUp         => { self.prompt_move_cursor_up(); },
                        pancurses::Input::KeyDown       => { self.prompt_move_cursor_down(); },
                        pancurses::Input::KeyBackspace  => { self.prompt_handle_backspace_key(); },
                        pancurses::Input::KeyDC         => { self.prompt_handle_delete_key() },
                        pancurses::Input::KeyHome       => { self.prompt_handle_home_key(); },
                        pancurses::Input::KeyEnd        => { self.prompt_handle_end_key(); },

                        pancurses::Input::KeyEnter      => { self.prompt_handle_enter_key(emulator, memory_system); },

                        pancurses::Input::Unknown(input_code) => {
                            match input_code {
                                155 => { self.prompt_handle_enter_key(emulator, memory_system); },        // Enter (keypad, w32)
                                _   => { },
                            }
                        },

                        pancurses::Input::Character(input_char) => {
                            if util::is_ascii_printable(input_char as u32) {
                                self.prompt_insert_char(util::ascii_to_printable_char(input_char as u8));
                            } else {
                                match input_char as u8 {
                                    0x08  => { self.prompt_handle_backspace_key(); },                    // Backspace (w32)
                                    0x0C  => { self.window.clearok(true); self.redraw_needed = true; },  // CTRL+L
                                    0x0D  => { self.prompt_handle_enter_key(emulator, memory_system); }, // Enter
                                    0x15  => { self.prompt_handle_ctrl_u(); },                           // CTRL+U
                                    _     => { },
                                }
                            }
                        },

                        _ => { },
                    }
                },
                None => {
                    break;
                },
            }
        }
    }
    fn handle_resize_event(&mut self) {
        let new_width  = self.window.get_max_x();
        let new_height = self.window.get_max_y();

        if (new_width  as usize) != self.screen_width  ||
           (new_height as usize) != self.screen_height {

            assert!(new_width >= 0 && new_height >= 0);
            self.screen_width  = new_width  as usize;
            self.screen_height = new_height as usize;

            if self.screen_width < MIN_SCREEN_WIDTH ||
               self.screen_height < MIN_SCREEN_HEIGHT {
                self.screen_too_small = true;
            } else {
                self.screen_too_small = false;
                self.prompt_scroll = 0;
                self.scroll_prompt_if_needed();
            }
            self.redraw_needed = true;
        }
    }
    pub fn execute_command(&mut self, input_str: &str, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {

        let command = match util::get_word(input_str, 1) {
                          Some(command_word) => { command_word.to_lowercase() },
                          None => { return },
                      };

        if command == "exit" || command == "quit" {
            emulator.exit_request = true;

        } else if command == "help" {
            self.help_command(input_str);

        } else if command == "messages" {
            self.messages_command(input_str);

        } else if command == "machine" {
            self.machine_command(input_str, emulator, memory_system);

        } else if command == "memory" {
            self.memory_command(input_str, memory_system);

        // Alias for "clear screen":
        } else if command == "clear" || command == "cls" {
            self.execute_command("messages clear all", emulator, memory_system)

        // Alias for "pause" and "unpause":
        } else if command == "pause" {
            self.execute_command("machine pause on", emulator, memory_system)

        } else if command == "unpause" {
            self.execute_command("machine pause off", emulator, memory_system)

        } else {
            self.emulator_message(format!("Unknown command: `{}'", command).as_str());
        }
    }
    fn help_command(&mut self, full_command_str: &str) {
        
        let command_lc = match util::get_word(full_command_str, 2) {
                             Some(command_text) => { command_text.to_lowercase() },
                             None => { "".to_owned() },
                         };

        if command_lc.is_empty() {
            self.emulator_message("Key bindings in the SDL2-based interface:");
            self.emulator_message("");
            self.emulator_message("    F1, Insert  - bindings for the `break' key.");
            self.emulator_message("    F2, Delete  - bindings for the `clear' key.");
            self.emulator_message("    F4          - pauses/unpauses emulation, alias for `machine pause toggle'.");
            self.emulator_message("    F5          - performs a full system reset, alias for `machine reset full'.");
            self.emulator_message("    F11         - toggles the full-screen mode.");
            self.emulator_message("");
            self.emulator_message("Available commands in the curses-based interface:");
            self.emulator_message("");
            self.emulator_message("    help        - shows information about other commands.");
            self.emulator_message("    messages    - manages the messages on the curses-based interface.");
            self.emulator_message("    machine     - allows you to change the state of the emulated machine.");
            self.emulator_message("    memory      - allows you to change the state of the memory system.");
            self.emulator_message("");
            self.emulator_message("    F1          - alias for `help', pressing F1 shows this message.");
            self.emulator_message("    clear, cls  - aliases for `messages clear all'.");
            self.emulator_message("    pause       - alias for `machine pause on'.");
            self.emulator_message("    unpause     - alias for `machine pause off'.");
            self.emulator_message("");
            self.emulator_message("Type `/help command' for more information about specific commands.");
        } else if command_lc == "help" {
            self.emulator_message("The `help' command is used to explain the commands that are available in the curses-based user interface of the emulator.");
            self.emulator_message("");
            self.emulator_message("For a list of commands, type `/help' with no argument.");
            self.emulator_message("");
            self.emulator_message("For more information about a specific command, type `/help command', where `command' is one of the comands returned by `/help'.");
        } else if command_lc == "messages" {
            self.emulator_message("The `messages' command has the following sub-commands:");
            self.emulator_message("");
            self.emulator_message("    messages show <machine|emulator>      - makes the given type of messages visible.");
            self.emulator_message("    messages hide <machine|emulator>      - makes the given type of messages invisible.");
            self.emulator_message("    messages toggle <machine|emulator>    - toggles the visibility of messages of the given type.");
            self.emulator_message("    messages clear <machine|emulator|all> - clears/removes messages of the given type.");
            self.emulator_message("");
            self.emulator_message("`emulator' messages are ones that are emitted by the emulator itself, `machine' messages are emitted by the emulated machine.");
        } else if command_lc == "machine" {
            self.emulator_message("The `machine' command has the following sub-commands:");
            self.emulator_message("");
            self.emulator_message("    machine power <on|off>        - powers the machine on or off.");
            self.emulator_message("    machine reset [full]          - performs a CPU reset, or a full reset.");
            self.emulator_message("    machine pause [on|off|toggle] - pauses or unpauses the machine.");
            self.emulator_message("    machine unpause               - alias for `machine pause off'.");
            self.emulator_message("");
            self.emulator_message("With no argument, `machine reset' performs a CPU reset, and `machine pause' pauses the machine's emulation.");
        } else if command_lc == "memory" {
            self.emulator_message("The `memory' command has the following sub-commands:");
            self.emulator_message("");
            self.emulator_message("    memory load <rom|ram> <file> [offset] - loads a file into either ram or rom.");
            self.emulator_message("    memory wipe <rom|ram|all>             - clears the contents of rom, ram, or both.");
            self.emulator_message("");
            self.emulator_message("The offset specifier in `memory load' can be in either decimal, octal, binary or hexadecimal notation.  The default is decimal, a prefix of 0b means binary, 0x means hexadecimal, 0 means octal, and a postfix of h means hexadecimal.");
            self.emulator_message("");
            self.emulator_message("In the current implementation, file names may not contain spaces and non-ascii characters.");
        } else if command_lc == "clear" || command_lc == "cls" {
            self.emulator_message(format!("The `{}' command is an alias for `messages clear all', see `/help messages' for more information.", command_lc).as_str());
        } else if command_lc == "pause" {
            self.emulator_message("The `pause' command is an alias for `machine pause on', see `/help machine' for more information.");
        } else if command_lc == "unpause" {
            self.emulator_message("The `unpause' command is an alias for `machine pause off', see `/help machine' for more information.");
        } else {
            self.emulator_message(format!("Unknown command `{}'.  See `/help' with no argument for a list of supported commands.", command_lc).as_str());
        }
    }
    fn messages_command(&mut self, full_command_str: &str) {

        let sub_command_lc = match util::get_word(full_command_str, 2) {
                                 Some(sub_command_text) => { sub_command_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };
        let parameter_lc = match util::get_word(full_command_str, 3) {
                               Some(parameter_text) => { parameter_text.to_lowercase() },
                               None => { "".to_owned() },
                           };

        if sub_command_lc == "show" {
            if parameter_lc == "machine" {
                self.show_machine_messages();
            } else if parameter_lc == "emulator" {
                self.show_emulator_messages();
            } else if parameter_lc.is_empty() {
                self.emulator_message(format!("The `messages {}' command requires a parameter, see: /help messages", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `messages {}' command, see: /help messages", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "hide" {
            if parameter_lc == "machine" {
                self.hide_machine_messages();
            } else if parameter_lc == "emulator" {
                self.hide_emulator_messages();
            } else if parameter_lc.is_empty() {
                self.emulator_message(format!("The `messages {}' command requires a parameter, see: /help messages", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `messages {}' command, see: /help messages", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "toggle" {
            if parameter_lc == "machine" {
                if self.machine_msg_shown {
                    self.hide_machine_messages();
                } else {
                    self.show_machine_messages();
                }
            } else if parameter_lc == "emulator" {
                if self.emulator_msg_shown {
                    self.hide_emulator_messages();
                } else {
                    self.show_emulator_messages();
                }
            } else if parameter_lc.is_empty() {
                self.emulator_message(format!("The `messages {}' command requires a parameter, see: /help messages", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `messages {}' command, see: /help messages", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "clear" {
            if parameter_lc == "machine" {
                self.clear_machine_messages();
            } else if parameter_lc == "emulator" {
                self.clear_emulator_messages();
            } else if parameter_lc == "all" {
                self.clear_all_messages();
            } else if parameter_lc.is_empty() {
                self.emulator_message(format!("The `messages {}' command requires a parameter, see: /help messages", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `messages {}' command, see: /help messages", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc.is_empty() {
            self.emulator_message("The `messages' command requires a sub-command, see: /help messages");
        } else {
            self.emulator_message(format!("Invalid sub-command `{}' for the `messages' command, see: /help messages", sub_command_lc).as_str());
        }
    }
    fn show_emulator_messages(&mut self) {
        if self.emulator_msg_shown {
            self.emulator_message("Emulator messages already visible.");
        } else {
            self.emulator_msg_shown = true;
            self.emulator_message("Emulator messages shown.");
        }
    }
    fn hide_emulator_messages(&mut self) {
        if !self.emulator_msg_shown {
            self.emulator_message("Emulator messages already hidden.");
        } else {
            self.emulator_msg_shown = false;
            self.emulator_message("Emulator messages hidden.");
        }
    }
    fn show_machine_messages(&mut self) {
        if self.machine_msg_shown {
            self.emulator_message("Machine messages already visible.");
        } else {
            self.machine_msg_shown = true;
            self.emulator_message("Machine messages shown.");
        }
    }
    fn hide_machine_messages(&mut self) {
        if !self.machine_msg_shown {
            self.emulator_message("Machine messages already hidden.");
        } else {
            self.machine_msg_shown = false;
            self.emulator_message("Machine messages hidden.");
        }
    }
    fn clear_machine_messages(&mut self) {
        let mut new_screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        for line in &self.screen_lines {
            match line.line_type {
                ScreenLineType::MachineMessage {..} => { },
                _ => {
                    new_screen_lines.push_back(ScreenLine {
                                                   line_type:    line.line_type.clone(),
                                                   line_content: line.line_content.to_owned(),
                                               });
                },
            }
        }
        self.screen_lines = new_screen_lines;
        self.emulator_message("Machine messages cleared.");
    }
    fn clear_emulator_messages(&mut self) {
        let mut new_screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        for line in &self.screen_lines {
            match line.line_type {
                ScreenLineType::EmulatorMessage => { },
                _ => {
                    new_screen_lines.push_back(ScreenLine {
                                                   line_type:    line.line_type.clone(),
                                                   line_content: line.line_content.to_owned(),
                                               });
                },
            }
        }
        self.screen_lines = new_screen_lines;
        self.emulator_message("Emulator messages cleared.");
    }
    fn clear_all_messages(&mut self) {
        self.screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        self.emulator_message("All messages cleared.");
    }
    fn machine_command(&mut self, full_command_str: &str, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        let sub_command_lc = match util::get_word(full_command_str, 2) {
                                 Some(sub_command_text) => { sub_command_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };
        let parameter_lc = match util::get_word(full_command_str, 3) {
                               Some(parameter_text) => { parameter_text.to_lowercase() },
                               None => { "".to_owned() },
                           };
        if sub_command_lc == "power" {
            if parameter_lc == "off" {
                self.power_off_machine(emulator, memory_system);
            } else if parameter_lc == "on" {
                self.power_on_machine(emulator, memory_system);
            } else if parameter_lc.is_empty() {
                self.emulator_message(format!("The `machine {}' command requires a parameter, see: /help machine", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `machine {}' command, see: /help machine", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "reset" {
            if parameter_lc == "full" {
                self.reset_machine_full(emulator, memory_system);
            } else if parameter_lc.is_empty() {
                self.reset_machine(emulator, memory_system);
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `machine {}' command, see: /help machine", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "pause" {
            if parameter_lc == "off" {
                self.unpause_machine(emulator);
            } else if parameter_lc == "on" || parameter_lc.is_empty() {
                self.pause_machine(emulator);
            } else if parameter_lc == "toggle" {
                if emulator.paused {
                    self.unpause_machine(emulator);
                } else {
                    self.pause_machine(emulator);
                }
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `machine {}' command, see: /help machine", parameter_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "unpause" {
            self.unpause_machine(emulator);
        } else if sub_command_lc.is_empty() {
            self.emulator_message("The `machine' command requires a sub-command, see: /help machine".to_owned().as_str());
        } else {
            self.emulator_message(format!("Invalid sub-command `{}' for the `machine' command, see: /help machine", sub_command_lc).as_str());
        }
    }
    fn power_off_machine(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        if !emulator.powered_on {
            self.emulator_message("The machine is already powered off.");
        } else {
            emulator.power_off(memory_system);
            self.collect_logged_messages(emulator, memory_system);
            self.emulator_message("Machine powered off.");
        }
    }
    fn power_on_machine(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        if emulator.powered_on {
            self.emulator_message("The machine is already powered on.");
        } else {
            emulator.power_on(memory_system);
            self.collect_logged_messages(emulator, memory_system);
            self.emulator_message("Machine powered on.");
        }
    }
    fn reset_machine_full(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        if emulator.powered_on {
            emulator.full_reset(memory_system);
            self.collect_logged_messages(emulator, memory_system);
            self.emulator_message("Full system reset performed.");
        } else {
            self.emulator_message("Cannot reset a powered-off machine.");
        }
    }
    fn reset_machine(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        if emulator.powered_on {
            emulator.reset(memory_system);
            self.collect_logged_messages(emulator, memory_system);
            self.emulator_message("System reset performed.");
        } else {
            self.emulator_message("Cannot reset a powered-off machine.");
        }
    }
    fn pause_machine(&mut self, emulator: &mut emulator::Emulator) {
        if emulator.paused {
            self.emulator_message("The machine emulation is already paused.");
        } else {
            emulator.paused = true;
            self.emulator_message("Machine emulation paused.");
        }
    }
    fn unpause_machine(&mut self, emulator: &mut emulator::Emulator) {
        if !emulator.paused {
            self.emulator_message("The machine emulation is already not paused.");
        } else {
            emulator.paused = false;
            self.emulator_message("Machine emulation unpaused.");
        }
    }
    fn memory_command(&mut self, full_command_str: &str, memory_system: &mut memory::MemorySystem) {
        let sub_command_lc = match util::get_word(full_command_str, 2) {
                                 Some(sub_command_text) => { sub_command_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };
        let parameter_1_lc = match util::get_word(full_command_str, 3) {
                                 Some(parameter_text) => { parameter_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };
        let parameter_2_lc = match util::get_word(full_command_str, 4) {
                                 Some(parameter_text) => { parameter_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };
        let parameter_3_lc = match util::get_word(full_command_str, 5) {
                                 Some(parameter_text) => { parameter_text.to_lowercase() },
                                 None => { "".to_owned() },
                             };

        if sub_command_lc == "load" {
            if parameter_1_lc == "ram" {
                if parameter_2_lc.is_empty() {
                    self.emulator_message("No file specified for loading into ram.");
                } else {
                    if parameter_3_lc.is_empty() {
                        memory_system.ram_chip.load_from_file(parameter_2_lc, 0x0000);
                    } else {
                        match util::parse_u32_from_str(parameter_3_lc.as_str()) {
                            Some(offset) => {
                                if offset > 0xFFFF {
                                    self.emulator_message(format!("Invalid starting offset `{}' specified for the `memory {}' command, see: /help memory", parameter_3_lc, sub_command_lc).as_str());
                                } else {
                                    memory_system.ram_chip.load_from_file(parameter_2_lc, offset as u16);
                                }
                            },
                            None => {
                                self.emulator_message(format!("Invalid starting offset `{}' specified for the `memory {}' command, see: /help memory", parameter_3_lc, sub_command_lc).as_str());
                            }
                        }
                    }
                }
            } else if parameter_1_lc == "rom" {
                if parameter_2_lc.is_empty() {
                    self.emulator_message("No file specified for loading into rom.");
                } else {
                    if parameter_3_lc.is_empty() {
                        memory_system.rom_chip.load_from_file(parameter_2_lc, 0x0000);
                    } else {
                        match util::parse_u32_from_str(parameter_3_lc.as_str()) {
                            Some(offset) => {
                                if offset > 0xFFFF {
                                    self.emulator_message(format!("Invalid starting offset `{}' specified for the `memory {}' command, see: /help memory", parameter_3_lc, sub_command_lc).as_str());
                                } else {
                                    memory_system.rom_chip.load_from_file(parameter_2_lc, offset as u16);
                                }
                            },
                            None => {
                                self.emulator_message(format!("Invalid starting offset `{}' specified for the `memory {}' command, see: /help memory", parameter_3_lc, sub_command_lc).as_str());
                            }
                        }
                    }
                }
            } else if parameter_1_lc.is_empty() {
                self.emulator_message(format!("The `memory {}' command requires a parameter, see: /help memory", sub_command_lc).as_str());
            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `memory {}' command, see: /help memory", parameter_1_lc, sub_command_lc).as_str());
            }
        } else if sub_command_lc == "wipe" {
            if parameter_1_lc == "ram" {
                memory_system.ram_chip.wipe();

            } else if parameter_1_lc == "rom" {
                memory_system.rom_chip.wipe();

            } else if parameter_1_lc == "all" {
                memory_system.ram_chip.wipe();
                memory_system.rom_chip.wipe();

            } else if parameter_1_lc.is_empty() {
                self.emulator_message(format!("The `memory {}' command requires a parameter, see: /help memory", sub_command_lc).as_str());

            } else {
                self.emulator_message(format!("Invalid parameter `{}' for the `memory {}' command, see: /help memory", parameter_1_lc, sub_command_lc).as_str());

            }
        } else if sub_command_lc.is_empty() {
            self.emulator_message("The `memory' command requires a sub-command, see: /help memory");
        } else {
            self.emulator_message(format!("Invalid sub-command `{}' for the `memory' command, see: /help memory", sub_command_lc).as_str());
        }
    }
    fn send_to_console(&mut self, _input_str: String, _emulator: &mut emulator::Emulator) {
        self.emulator_message("Serial console interface not yet implemented.");
    }
    pub fn add_screen_line(&mut self, line_content: String, line_type: ScreenLineType) {
        enum Action {
            SimplyAppend,
            AppendAndSwap,
            AppendToLast,
        };
        let action: Action = match self.screen_lines.front() {
            None => {
                Action::SimplyAppend
            },
            Some(last_line) => {
                match last_line.line_type {
                    ScreenLineType::EmulatorMessage => {
                        Action::SimplyAppend
                    },
                    ScreenLineType::MachineMessage { complete: last_is_complete } => {
                        if last_is_complete {
                            Action::SimplyAppend
                        } else {
                            match line_type {
                                ScreenLineType::EmulatorMessage => {
                                    Action::AppendAndSwap
                                },
                                ScreenLineType::MachineMessage {..} => {
                                    Action::AppendToLast
                                },
                            }
                        }
                    },
                }
            },
        };

        self.screen_lines.truncate(self.max_screen_lines - 1);
        match action {
            Action::SimplyAppend => {
                let new_line = ScreenLine {
                                   line_type:    line_type,
                                   line_content: line_content,
                               };
                if self.lines_scroll > 0 {
                    self.lines_scroll += new_line.physical_lines(self.screen_width);
                }
                self.screen_lines.push_front(new_line);
            },
            Action::AppendAndSwap => {
                let new_line = ScreenLine {
                                   line_type:    line_type,
                                   line_content: line_content,
                               };
                if self.lines_scroll > 0 {
                    self.lines_scroll += new_line.physical_lines(self.screen_width);
                }
                self.screen_lines.push_front(new_line);
                self.screen_lines.swap(0, 1);
            },
            Action::AppendToLast => {
                let last_line_mut = self.screen_lines.front_mut().expect(".expect() call: If it didn't exist, we wouldn't be appending to it");
                let last_line_old_phys = last_line_mut.physical_lines(self.screen_width);

                last_line_mut.line_content.push_str(line_content.as_str());
                last_line_mut.line_type = line_type;

                let last_line_new_phys = last_line_mut.physical_lines(self.screen_width);
                let phys_lines_added = last_line_new_phys - last_line_old_phys;

                if (phys_lines_added > 0) && (self.lines_scroll > 0) {
                    self.lines_scroll += phys_lines_added;
                }
            },
        }
        if self.lines_scroll > 0 {
            self.lines_added_scrolled_up = true;
        }

        self.redraw_needed = true;
    }
    pub fn emulator_message(&mut self, line_content: &str) {
        // For the sake of simplicity, we only allow ASCII:
        let mut filtered_line_content = String::new();

        for character in line_content.chars() {
            if util::is_ascii_printable(character as u32) {
                filtered_line_content.push(util::ascii_to_printable_char(character as u8));
            } else {
                filtered_line_content.push(util::ascii_to_printable_char(0));
            }
        }

        self.add_screen_line(filtered_line_content, ScreenLineType::EmulatorMessage);
    }
    pub fn machine_line_add_char(&mut self, char_to_add: char) {
        self.add_screen_line(format!("{}", char_to_add), ScreenLineType::MachineMessage { complete: false });
    }
    pub fn machine_line_finalize(&mut self) {
        self.add_screen_line("".to_owned(), ScreenLineType::MachineMessage { complete: true });
    }
    fn scroll_lines_up(&mut self) {
        // The user can request to scroll as much as they wish, the lines rendering
        // routine will then normalize this value.
        self.lines_scroll += self.screen_height / 2;
        self.redraw_needed = true;
    }
    fn scroll_lines_down(&mut self) {
        if self.lines_scroll >= (self.screen_height / 2) {
            self.lines_scroll -= self.screen_height / 2;
        } else {
            self.lines_scroll = 0;
        }

        if self.lines_scroll == 0 {
            self.lines_added_scrolled_up = false;
        }

        self.redraw_needed = true;
    }
    fn scroll_prompt_if_needed(&mut self) {
        // If the cursor is outside of the visible range of the prompt, we need
        // to scroll it.
        //
        if (self.prompt_curs_pos + PROMPT_TEXT_OFFSET >= self.screen_width + self.prompt_scroll) ||
           (self.prompt_curs_pos < self.prompt_scroll) {

            self.prompt_scroll = 0;
            while self.prompt_curs_pos + PROMPT_TEXT_OFFSET - self.prompt_scroll >= self.screen_width {
                self.prompt_scroll += self.screen_width / 2;
            }
        }
    }
    fn prompt_insert_char(&mut self, character: char) {
        if self.prompt_history_pos > 0 {
            self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
            self.prompt_history_pos = 0;
        }
        self.prompt_text.insert(self.prompt_curs_pos, character);
        self.redraw_needed = true;

        self.prompt_curs_pos += 1;
        self.scroll_prompt_if_needed();
    }
    fn prompt_move_cursor_left(&mut self) {
        if self.prompt_curs_pos > 0 {
            self.prompt_curs_pos -= 1;
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_right(&mut self) {
        let prompt_text_len = match self.prompt_history_pos {
            0 => { self.prompt_text.len() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].len() },
        };
        if self.prompt_curs_pos < prompt_text_len {
            self.prompt_curs_pos += 1;
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_up(&mut self) {
        if self.prompt_history_pos < self.prompt_history.len() {
            self.prompt_history_pos += 1;
            self.prompt_curs_pos = match self.prompt_history_pos {
                                       0 => { self.prompt_text.as_str().len() },
                                       _ => { self.prompt_history[self.prompt_history_pos - 1].len() },
                                   };
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_down(&mut self) {
        if self.prompt_history_pos > 0 {
            self.prompt_history_pos -= 1;
            self.prompt_curs_pos = match self.prompt_history_pos {
                                       0 => { self.prompt_text.as_str().len() },
                                       _ => { self.prompt_history[self.prompt_history_pos - 1].len() },
                                   };
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_backspace_key(&mut self) {
        if self.prompt_curs_pos > 0 {
            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }
            self.prompt_text.remove(self.prompt_curs_pos - 1);
            self.prompt_curs_pos -= 1;
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_delete_key(&mut self) {
        let prompt_text_len = match self.prompt_history_pos {
            0 => { self.prompt_text.len() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].len() },
        };
        if self.prompt_curs_pos < prompt_text_len {
            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }
            self.prompt_text.remove(self.prompt_curs_pos);
            self.redraw_needed = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_ctrl_u(&mut self) {
        if self.prompt_curs_pos > 0 {
            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }
            let to_keep = {
                let (_, upper_half) = self.prompt_text.as_str().split_at(self.prompt_curs_pos);
                upper_half.to_owned()
            };

            self.prompt_text = to_keep;
            self.prompt_curs_pos = 0;

            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        }
    }
    fn prompt_handle_home_key(&mut self) {
        if self.prompt_curs_pos != 0 {
            self.prompt_curs_pos = 0;
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        }
    }
    fn prompt_handle_end_key(&mut self) {
        let prompt_text_len = match self.prompt_history_pos {
            0 => { self.prompt_text.len() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].len() },
        };
        if self.prompt_curs_pos != prompt_text_len {
            self.prompt_curs_pos = prompt_text_len;
            self.scroll_prompt_if_needed();
            self.redraw_needed = true;
        }
    }
    fn prompt_handle_enter_key(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        let entered_text = match self.prompt_history_pos {
            0 => { self.prompt_text.clone() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].clone() },
        };

        self.prompt_add_to_history(entered_text.as_str());
        self.prompt_text = "".to_owned();
        self.prompt_curs_pos = 0;
        self.prompt_history_pos = 0;
        self.scroll_prompt_if_needed();
        self.redraw_needed = true;

        if (entered_text.len() > 0) && (entered_text.chars().next().expect(".expect() call: Somehow there isn't a first character even though entered_text.len() > 0 evaluated to true") == '/') {
            let (_, command_str) = entered_text.as_str().split_at(1);

            if (command_str.len() > 0) && (command_str.chars().next().expect(".expect() call: Somehow there isn't a first character even though command_str.len() > 0 evaluated to true") == '/') {
                 self.send_to_console(command_str.to_owned(), emulator);
            } else {
                self.execute_command(command_str, emulator, memory_system);
            }
        } else {
            self.send_to_console(entered_text, emulator);
        }
    }
    fn prompt_add_to_history(&mut self, to_add: &str) {
        // Is this line identical to the last one in history?  If yes, ignore.
        match self.prompt_history.front() {
            Some(last_line) => {
                if to_add == last_line {
                    return;
                }
            },
            None => { },
        }

        self.prompt_history.truncate(self.prompt_history_max_entries - 1);
        self.prompt_history.push_front(to_add.to_owned());
    }
    pub fn collect_logged_messages(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {

        if emulator.messages_available() {
            for message in emulator.collect_messages() {
                self.emulator_message(message.as_str());
            }
        }

        if memory_system.messages_available() {
            for message in memory_system.collect_messages() {
                self.emulator_message(message.as_str());
            }
        }
    }
    pub fn update_screen(&mut self, emulator: &emulator::Emulator) {
        if self.redraw_needed
            || self.status_displayed_halted != emulator.cpu.halted {

            self.window.erase();

            if self.screen_too_small {
                self.window.mv(0, 0);
                self.window.printw(format!("Screen too small, minimum size is {} rows, {} cols.", MIN_SCREEN_HEIGHT, MIN_SCREEN_WIDTH).as_ref());
            } else {
                self.render_lines();
                self.render_status_strips(emulator);
                self.render_prompt();
            }

            self.window.refresh();
            self.redraw_needed = false;
        }
    }
    // Description:
    //
    // The following routine draws the text area, the "lines", of the console
    // window.  It draws them from bottom to top.
    //
    fn render_lines(&mut self) {
        let avail_phys_lines = self.screen_height - LINES_BOTTOM_OFFSET - LINES_TOP_OFFSET;
        let mut phys_lines_to_draw = 0;
        let mut phys_lines_to_scroll_over = 0;

        for virt_line in self.screen_lines.iter() {
            // Skip lines which aren't to be shown:
            match virt_line.line_type {
                ScreenLineType::EmulatorMessage => {
                    if !self.emulator_msg_shown {
                        continue;
                    }
                },
                ScreenLineType::MachineMessage {..} => {
                    if !self.machine_msg_shown {
                        continue;
                    }
                },
            }
            let cur_line_phys_lines = virt_line.physical_lines(self.screen_width);

            if phys_lines_to_draw < avail_phys_lines {
                phys_lines_to_draw += cur_line_phys_lines;
            } else if phys_lines_to_scroll_over < self.lines_scroll {
                if phys_lines_to_draw > avail_phys_lines {
                    phys_lines_to_scroll_over = phys_lines_to_draw - avail_phys_lines;
                    phys_lines_to_draw -= phys_lines_to_scroll_over;
                }
                phys_lines_to_scroll_over += cur_line_phys_lines;
            } else {
                break;
            }
        }

        // Sanitize the lines_scroll variable:
        if self.lines_scroll > phys_lines_to_scroll_over {
            self.lines_scroll = phys_lines_to_scroll_over;
        }

        if phys_lines_to_draw > 0 {
            let mut y_pos = (avail_phys_lines as i32) - 1 + (LINES_TOP_OFFSET as i32);
            if avail_phys_lines > phys_lines_to_draw {
                y_pos -= (avail_phys_lines as i32) - (phys_lines_to_draw as i32);
            }

            for virt_line in self.screen_lines.iter() {
                // Skip lines which aren't to be shown:
                match virt_line.line_type {
                    ScreenLineType::EmulatorMessage => {
                        if !self.emulator_msg_shown {
                            continue;
                        }
                    },
                    ScreenLineType::MachineMessage {..} => {
                        if !self.machine_msg_shown {
                            continue;
                        }
                    },
                }
                let mut cur_line_phys_lines = virt_line.physical_lines(self.screen_width);
                let cur_line_text;

                if phys_lines_to_scroll_over >= cur_line_phys_lines {
                    phys_lines_to_scroll_over -= cur_line_phys_lines;
                    continue;
                } else if phys_lines_to_scroll_over > 0 {
                    cur_line_phys_lines -= phys_lines_to_scroll_over;
                    phys_lines_to_scroll_over = 0;

                    cur_line_text = {
                        let (to_print, _) = virt_line.line_content.as_str().split_at(cur_line_phys_lines * self.screen_width);
                        to_print
                    };
                } else {
                    cur_line_text = virt_line.line_content.as_str();
                }
                let cur_line_phys_lines = cur_line_phys_lines;

                let color_pair = match virt_line.line_type {
                    ScreenLineType::EmulatorMessage     => { COLOR_PAIR_EMSG },
                    ScreenLineType::MachineMessage {..} => { COLOR_PAIR_MMSG },
                };

                let new_y_pos = y_pos - (cur_line_phys_lines as i32) + 1;

                if new_y_pos < LINES_TOP_OFFSET as i32 {
                    let phys_lines_to_skip = ((LINES_TOP_OFFSET as i32) - new_y_pos) as usize;

                    self.window.mv(LINES_TOP_OFFSET as i32, 0);
                    let (_, to_print) = cur_line_text.split_at((phys_lines_to_skip * self.screen_width) - 1);
                    self.window.attron(pancurses::colorpair::ColorPair(color_pair));
                    self.window.printw(to_print);
                    self.window.attroff(pancurses::colorpair::ColorPair(color_pair));

                    break;
                } else {
                    y_pos = new_y_pos;

                    self.window.mv(y_pos, 0);
                    self.window.attron(pancurses::colorpair::ColorPair(color_pair));
                    self.window.printw(cur_line_text);
                    self.window.attroff(pancurses::colorpair::ColorPair(color_pair));

                    y_pos -= 1;
                    if y_pos < LINES_TOP_OFFSET as i32 {
                        break;
                    }
                }
            }
        }
    }
    fn render_status_strips(&mut self, emulator: &emulator::Emulator) {

        // Update the cached values:
        self.status_displayed_halted = emulator.cpu.halted;

        // Color the strips:
        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
        self.window.mv(TOP_STRIP_TOP_OFFSET as i32, 0);
        self.window.hline(0x20, self.screen_width as i32);

        self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, 0);
        self.window.hline(0x20, self.screen_width as i32);

        // Write in some text:
        self.window.mv(TOP_STRIP_TOP_OFFSET as i32, 1);
        self.window.printw(format!("{} v{} - TRS-80 Model I emulator", PROGRAM_NAME, PROGRAM_VERSION).as_str());
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));

        self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, 1);

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        self.window.printw("[");
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

        self.window.attron(pancurses::A_BOLD);
        if emulator.powered_on {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
            self.window.printw("power on");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
        } else {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_RED));
            self.window.printw("power off");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_RED));
        }
        self.window.attroff(pancurses::A_BOLD);

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        self.window.printw("]");
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

        if emulator.powered_on || emulator.paused {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            self.window.printw(" ");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));

            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
            self.window.printw("[");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

            if emulator.paused {
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
                self.window.printw("paused");
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            } else if emulator.cpu.halted {
                self.window.attron(pancurses::A_BOLD);
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
                self.window.printw("halted");
                self.window.attroff(pancurses::A_BOLD);
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            } else {
                self.window.attron(pancurses::A_BOLD);
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
                self.window.printw("running");
                self.window.attroff(pancurses::A_BOLD);
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
            }

            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
            self.window.printw("]");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        }

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
        if self.lines_added_scrolled_up {
            self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, (self.screen_width as i32) - 1 - 10);
            self.window.printw("-- more --");
        }
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
    }
    fn render_prompt(&mut self) {
        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_PROMPT));

        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, 0);
        self.window.printw("> ");

        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, PROMPT_TEXT_OFFSET as i32);
        let current_prompt_text = match self.prompt_history_pos {
            0 => { self.prompt_text.as_str() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].as_str() },
        };
        let (_, upper_half) = current_prompt_text.split_at(self.prompt_scroll);
        let to_print = if upper_half.len() < (self.screen_width - PROMPT_TEXT_OFFSET) {
                           upper_half
                       } else {
                           let (to_print, _) = upper_half.split_at(self.screen_width - PROMPT_TEXT_OFFSET);
                           to_print
                       };
        self.window.printw(to_print);

        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, (self.prompt_curs_pos + PROMPT_TEXT_OFFSET - self.prompt_scroll) as i32);

        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_PROMPT));
    }
    pub fn enter_key_to_close_on_windows() {
        match cfg!(target_os = "windows") {
            false => { },
            true  => {
                println!("");
                print!("Press Enter to close the program... ");

                // Make sure the message is actually displayed.
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                let _ = handle.flush();

                // Wait for the user to press the Enter key.
                let mut in_char: [u8; 1] = [0; 1];
                let stdin = io::stdin();
                let mut handle = stdin.lock();
                let _ = handle.read(&mut in_char);
            },
        };
    }
    pub fn attach_panic_handler() {
        panic::set_hook(Box::new(|panic_info| {
            pancurses::endwin();
            eprintln!("");

            match panic_info.location() {
                Some(location) => {
                    eprint!("Panic occured in `{}', line {}: ", location.file(), location.line());
                },
                None => {
                    eprint!("Panic occured: ");
                },
            }
            let panic_message = match panic_info.payload().downcast_ref::<&'static str>() {
                Some(string_message) => string_message,
                None => match panic_info.payload().downcast_ref::<String>() {
                    Some(string_message) => string_message.as_str(),
                    None => "Unknown",
                }
            };
            eprintln!("{}.", panic_message);
            UserInterface::enter_key_to_close_on_windows();
        }));
    }
}

impl Drop for UserInterface {
    fn drop(&mut self) {
        pancurses::endwin();
    }
}
