// Copyright (c) 2018, 2023 Marek Benc <dusxmt@gmx.com>
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

use pancurses;

use std::collections::VecDeque;
use std::io;
use std::io::Read;
use std::io::Write;
use std::ffi::OsStr;
use std::path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::emulator::{EmulatorCommand, EmulatorCassetteCommand, EmulatorConfigCommand, EmulatorStatus};
use trs80m1_rs_core::cassette;
use crate::util;

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


// The following set of enums describe the possible commands the user may issue
// on the application's prompt:
enum HelpEntry {
    Help,
    Messages,
    Machine,
    Memory,
    Cassette,
    Config,
    Exit,
    Alias { alias_name: String, aliased_name: String, help_entry: String },
    Default,
}

enum MessagesSubCommandArgExclusive {
    Emulator,
    Machine,
}
enum MessagesSubCommandArgInclusive {
    Emulator,
    Machine,
    Both,
}
enum MessagesSubCommand {
    Show   (MessagesSubCommandArgExclusive),
    Hide   (MessagesSubCommandArgExclusive),
    Toggle (MessagesSubCommandArgExclusive),
    Clear  (MessagesSubCommandArgInclusive),
}

enum PauseType {
    Pause,
    Unpause,
    Toggle,
}
enum MachineSubCommand {
    Power { new_state:  bool },
    Reset { full_reset: bool },
    Restore,
    SwitchRom(u32),
    Pause(PauseType),
}

enum MemorySubCommandArgExclusive {
    RAM,
    ROM,
}
enum MemorySubCommandArgInclusive {
    RAM,
    ROM,
    Both,
}
enum MemorySubCommand {
    Load { device: MemorySubCommandArgExclusive, path: path::PathBuf, offset: u16 },
    Wipe { device: MemorySubCommandArgInclusive },
}

enum ParsedUserCommand {
    Help     (HelpEntry),
    Messages (MessagesSubCommand),
    Machine  (MachineSubCommand),
    Memory   (MemorySubCommand),
    Cassette (EmulatorCassetteCommand),
    Config   (EmulatorConfigCommand),

    CommandMissingParameter  { sup_command_name: String, sub_command_name: String, parameter_desc: String, parameter_desc_ia: String },
    CommandMissingSubcommand { sup_command_name: String },
    InvalidCommand           { command_name: String },
    InvalidSubCommand        { sup_command_name: String, sub_command_name: String },
    InvalidParameter         { sup_command_name: String, sub_command_name: String, parameter_text: String, parameter_desc: String },
}

impl ParsedUserCommand {
    pub fn parse(command_string: &str) -> ParsedUserCommand {
        let (command, command_raw) = match util::get_word(command_string, 1) {
                          Some(main_command_text) => { (main_command_text.to_lowercase(), main_command_text.to_owned()) },
                          None => { panic!("Command string empty"); },
                      };
        let sub_command = match util::get_word(command_string, 2) {
                              Some(sub_command_text) => { Some((sub_command_text.to_lowercase(), sub_command_text)) },
                              None => { None },
                          };
        let parameter_1 = match util::get_word(command_string, 3) {
                              Some(parameter_text) => { Some((parameter_text.to_lowercase(), parameter_text)) },
                              None => { None },
                          };
        let parameter_2 = match util::get_word(command_string, 4) {
                              Some(parameter_text) => { Some((parameter_text.to_lowercase(), parameter_text)) },
                              None => { None },
                          };
        let parameter_3 = match util::get_word(command_string, 5) {
                              Some(parameter_text) => { Some((parameter_text.to_lowercase(), parameter_text)) },
                              None => { None },
                          };

        if command == "help" {
            match sub_command {
                Some ((sub_command, sub_command_raw)) => {
                    if sub_command == "help" {
                        ParsedUserCommand::Help(HelpEntry::Help)
                    } else if sub_command == "messages" {
                        ParsedUserCommand::Help(HelpEntry::Messages)
                    } else if sub_command == "machine" {
                        ParsedUserCommand::Help(HelpEntry::Machine)
                    } else if sub_command == "memory" {
                        ParsedUserCommand::Help(HelpEntry::Memory)
                    } else if sub_command == "cassette" {
                        ParsedUserCommand::Help(HelpEntry::Cassette)
                    } else if sub_command == "config" {
                        ParsedUserCommand::Help(HelpEntry::Config)
                    } else if sub_command == "exit" || sub_command == "quit" {
                        ParsedUserCommand::Help(HelpEntry::Exit)
                    } else if sub_command == "clear" || sub_command == "cls" {
                        ParsedUserCommand::Help(HelpEntry::Alias { alias_name: sub_command, aliased_name: "messages clear all".to_owned(), help_entry: "messages".to_owned() })
                    } else if sub_command == "pause" {
                        ParsedUserCommand::Help(HelpEntry::Alias { alias_name: sub_command, aliased_name: "machine pause on".to_owned(), help_entry: "machine".to_owned() })
                    } else if sub_command == "unpause" {
                        ParsedUserCommand::Help(HelpEntry::Alias { alias_name: sub_command, aliased_name: "machine pause off".to_owned(), help_entry: "machine".to_owned() })
                    } else {
                        ParsedUserCommand::InvalidCommand { command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::Help(HelpEntry::Default)
                },
            }
        } else if command == "messages" {
            match sub_command {
                Some ((sub_command, sub_command_raw)) => {
                    if sub_command == "show" {
                        let (selection, selection_raw) = match parameter_1 {
                                                             Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                             None => {
                                                                 return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "selection".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                             },
                                                         };
                        if selection == "machine" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Show(MessagesSubCommandArgExclusive::Machine))
                        } else if selection == "emulator" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Show(MessagesSubCommandArgExclusive::Emulator))
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: selection_raw, parameter_desc: "selection".to_owned() }
                        }
                    } else if sub_command == "hide" {
                        let (selection, selection_raw) = match parameter_1 {
                                                             Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                             None => {
                                                                 return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "selection".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                             },
                                                         };
                        if selection == "machine" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Hide(MessagesSubCommandArgExclusive::Machine))
                        } else if selection == "emulator" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Hide(MessagesSubCommandArgExclusive::Emulator))
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: selection_raw, parameter_desc: "selection".to_owned() }
                        }
                    } else if sub_command == "toggle" {
                        let (selection, selection_raw) = match parameter_1 {
                                                             Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                             None => {
                                                                 return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "selection".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                             },
                                                         };
                        if selection == "machine" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Toggle(MessagesSubCommandArgExclusive::Machine))
                        } else if selection == "emulator" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Toggle(MessagesSubCommandArgExclusive::Emulator))
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: selection_raw, parameter_desc: "selection".to_owned() }
                        }
                    } else if sub_command == "clear" {
                        let (selection, selection_raw) = match parameter_1 {
                                                             Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                             None => {
                                                                 return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "selection".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                             },
                                                         };
                        if selection == "machine" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Clear(MessagesSubCommandArgInclusive::Machine))
                        } else if selection == "emulator" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Clear(MessagesSubCommandArgInclusive::Emulator))
                        } else if selection == "all" {
                            ParsedUserCommand::Messages(MessagesSubCommand::Clear(MessagesSubCommandArgInclusive::Both))
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: selection_raw, parameter_desc: "selection".to_owned() }
                        }
                    } else {
                        ParsedUserCommand::InvalidSubCommand { sup_command_name: command, sub_command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::CommandMissingSubcommand { sup_command_name: command }
                },
            }
        } else if command == "machine" {
            match sub_command {
                Some ((sub_command, sub_command_raw)) => {
                    if sub_command == "power" {
                        let (action, action_raw) = match parameter_1 {
                                                       Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                       None => {
                                                           return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "action".to_owned(), parameter_desc_ia: "an".to_owned() };
                                                       },
                                                   };
                        if action == "on" {
                            ParsedUserCommand::Machine(MachineSubCommand::Power { new_state: true })
                        } else if action == "off" {
                            ParsedUserCommand::Machine(MachineSubCommand::Power { new_state: false })
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: action_raw, parameter_desc: "action".to_owned() }
                        }
                    } else if sub_command == "reset" {
                        let (type_str, type_str_raw) = match parameter_1 {
                                                           Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                           None => { ("cpu".to_owned(), "cpu".to_owned()) },
                                                       };
                        if type_str == "cpu" {
                            ParsedUserCommand::Machine(MachineSubCommand::Reset { full_reset: false })
                        } else if type_str == "full" {
                            ParsedUserCommand::Machine(MachineSubCommand::Reset { full_reset: true })
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: type_str_raw, parameter_desc: "reset type".to_owned() }
                        }
                    } else if sub_command == "restore" {
                        ParsedUserCommand::Machine(MachineSubCommand::Restore)
                    } else if sub_command == "switch-rom" {
                        let rom_nr_str = match parameter_1 {
                                                               Some((_, parameter_1_raw)) => { parameter_1_raw },
                                                               None => {
                                                                   return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "ROM number".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                               },
                                                           };

                        let rom_nr = match util::parse_u32_from_str(rom_nr_str.as_str()) {
                                         Some(offset_val) => {
                                             offset_val
                                         },
                                         None => {
                                             return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: rom_nr_str, parameter_desc: "ROM number".to_owned() };
                                         },
                                     };
                        if rom_nr < 1 || rom_nr > 3 {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: rom_nr_str, parameter_desc: "ROM number".to_owned() }
                        } else {
                            ParsedUserCommand::Machine(MachineSubCommand::SwitchRom(rom_nr))
                        }

                    } else if sub_command == "pause" {
                        let (type_str, type_str_raw) = match parameter_1 {
                                                           Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                           None => { ("on".to_owned(), "on".to_owned()) },
                                                       };
                        if type_str == "on" {
                            ParsedUserCommand::Machine(MachineSubCommand::Pause(PauseType::Pause))
                        } else if type_str == "off" {
                            ParsedUserCommand::Machine(MachineSubCommand::Pause(PauseType::Unpause))
                        } else if type_str == "toggle" {
                            ParsedUserCommand::Machine(MachineSubCommand::Pause(PauseType::Toggle))
                        } else {
                            ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: type_str_raw, parameter_desc: "pause type".to_owned() }
                        }
                    } else if sub_command == "unpause" {
                        ParsedUserCommand::Machine(MachineSubCommand::Pause(PauseType::Unpause))
                    } else {
                        ParsedUserCommand::InvalidSubCommand { sup_command_name: command, sub_command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::CommandMissingSubcommand { sup_command_name: command }
                },
            }
        } else if command == "memory" {
            match sub_command {
                Some((sub_command, sub_command_raw)) => {
                    if sub_command == "load" {
                        let (device_str, device_str_raw) = match parameter_1 {
                                                               Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                               None => {
                                                                   return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "device".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                               },
                                                           };
                        let device = if device_str == "ram" {
                            MemorySubCommandArgExclusive::RAM
                        } else if device_str == "rom" {
                            MemorySubCommandArgExclusive::ROM
                        } else {
                            return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: device_str_raw, parameter_desc: "device".to_owned() }
                        };
                        let file_name = match parameter_2 {
                                            Some((_, parameter_2_raw)) => { (parameter_2_raw.as_str().as_ref() as &path::Path).to_owned() },
                                            None => {
                                                return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "file name".to_owned(), parameter_desc_ia: "a".to_owned() };
                                            },
                                        };
                        let offset_str = match parameter_3 {
                                             Some((_, parameter_3_raw)) => { parameter_3_raw },
                                             None => { "0".to_owned() },
                                         };
                        let offset = match util::parse_u32_from_str(offset_str.as_str()) {
                                         Some(offset_val) => {
                                             if offset_val > 0xFFFF {
                                                 return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: offset_str, parameter_desc: "offset".to_owned() };
                                             } else {
                                               offset_val as u16
                                             }
                                         },
                                         None => {
                                             return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: offset_str, parameter_desc: "offset".to_owned() };
                                         },
                                     };
                        ParsedUserCommand::Memory(MemorySubCommand::Load { device: device, path: file_name, offset: offset })
                    } else if sub_command == "wipe" {
                        let (device_str, device_str_raw) = match parameter_1 {
                                                               Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                               None => {
                                                                   return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "device".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                               },
                                                           };
                        let device = if device_str == "ram" {
                            MemorySubCommandArgInclusive::RAM
                        } else if device_str == "rom" {
                            MemorySubCommandArgInclusive::ROM
                        } else if device_str == "all" {
                            MemorySubCommandArgInclusive::Both
                        } else {
                            return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: device_str_raw, parameter_desc: "device".to_owned() };
                        };
                        ParsedUserCommand::Memory(MemorySubCommand::Wipe { device: device })
                    } else {
                        ParsedUserCommand::InvalidSubCommand { sup_command_name: command, sub_command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::CommandMissingSubcommand { sup_command_name: command }
                },
            }
        } else if command == "cassette" {
            match sub_command {
                Some((sub_command, sub_command_raw)) => {
                    if sub_command == "insert" {
                        let (format_str, format_str_raw) = match parameter_1 {
                                                               Some((parameter_1, parameter_1_raw)) => { (parameter_1, parameter_1_raw) },
                                                               None => {
                                                                   return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "format".to_owned(), parameter_desc_ia: "a".to_owned() };
                                                               },
                                                           };
                        let format = if format_str == "cas" {
                            cassette::Format::CAS
                        } else if format_str == "cpt" {
                            cassette::Format::CPT
                        } else {
                            return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: format_str_raw, parameter_desc: "format".to_owned() };
                        };
                        match util::get_starting_at_word(command_string, 4) {
                            Some(file) => {
                                ParsedUserCommand::Cassette(EmulatorCassetteCommand::Insert { format: format, file: file })
                            },
                            None => {
                                ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "file".to_owned(), parameter_desc_ia: "a".to_owned() }
                            },
                        }
                    } else if sub_command == "seek" {
                        let position_str = match parameter_1 {
                                               Some((_, parameter_1_raw)) => { parameter_1_raw },
                                               None => {
                                                   return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "position".to_owned(), parameter_desc_ia: "a".to_owned() };
                                               },
                                           };
                        match position_str.parse::<usize>() {
                            Ok(position) => {
                                ParsedUserCommand::Cassette(EmulatorCassetteCommand::Seek { position: position })
                            },
                            Err(_) => {
                                ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: position_str, parameter_desc: "position".to_owned() }
                            },
                        }
                    } else if sub_command == "eject" {
                        ParsedUserCommand::Cassette(EmulatorCassetteCommand::Eject)
                    } else if sub_command == "erase" {
                        ParsedUserCommand::Cassette(EmulatorCassetteCommand::Erase)
                    } else if sub_command == "rewind" {
                        ParsedUserCommand::Cassette(EmulatorCassetteCommand::Rewind)
                    } else {
                        ParsedUserCommand::InvalidSubCommand { sup_command_name: command, sub_command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::CommandMissingSubcommand { sup_command_name: command }
                },
            }
        } else if command == "config" {
            match sub_command {
                Some ((sub_command, sub_command_raw)) => {
                    if sub_command == "list" {
                        ParsedUserCommand::Config(EmulatorConfigCommand::List)
                    } else if sub_command == "show" {
                        let entry_specifier = match parameter_1 {
                                                  Some((_, parameter_1_raw)) => { parameter_1_raw },
                                                  None => {
                                                      return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "entry specifier".to_owned(), parameter_desc_ia: "an".to_owned() };
                                                  },
                                              };
                        ParsedUserCommand::Config(EmulatorConfigCommand::Show { entry_specifier: entry_specifier })
                    } else if sub_command == "change" {
                        let entry_specifier = match parameter_1 {
                                                  Some((_, parameter_1_raw)) => { parameter_1_raw },
                                                  None => {
                                                      return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "entry specifier".to_owned(), parameter_desc_ia: "an".to_owned() };
                                                  },
                                              };
                        let equals_sign = match parameter_2 {
                                              Some((_, parameter_2_raw)) => { parameter_2_raw },
                                              None => {
                                                  return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "new value specifier".to_owned(), parameter_desc_ia: "a".to_owned() };
                                              },
                                          };
                        if equals_sign.chars().next().expect("Some((_, parameter_2_raw)) implies non-zero length") != '=' {
                            return ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: equals_sign, parameter_desc: "new value specifier".to_owned() };
                        }
                        ParsedUserCommand::Config(EmulatorConfigCommand::Change { entry_specifier: entry_specifier, invocation_text: command_string.to_owned() })
                    } else {
                        ParsedUserCommand::InvalidSubCommand { sup_command_name: command, sub_command_name: sub_command_raw }
                    }
                },
                None => {
                    ParsedUserCommand::CommandMissingSubcommand { sup_command_name: command }
                },
            }
        } else {
            ParsedUserCommand::InvalidCommand { command_name: command_raw }
        }
    }
}


pub struct UserInterface {
    window:                      pancurses::Window,
    exit_request:                bool,
    logic_core_thread_running:   bool,
    video_thread_running:        bool,

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

    cpu_halted:                  bool,
    machine_powered_on:          bool,
    machine_paused:              bool,
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
                                     exit_request:                false,
                                     logic_core_thread_running:   false,
                                     video_thread_running:        false,

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

                                     cpu_halted:                  false,
                                     machine_powered_on:          false,
                                     machine_paused:              false,
                                 };
        user_interface.handle_resize_event();

        Some(user_interface)
    }
    pub fn run(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, emu_stat_rx: &mpsc::Receiver<EmulatorStatus>, msg_source: &util::MessageLogger) {
        let sleep_len = Duration::from_millis(10);
        let mut waiting_for_logic_core_thread = true;
        let mut waiting_for_video_thread = true;

        while !self.exit_request || ((waiting_for_logic_core_thread || self.logic_core_thread_running) || (waiting_for_video_thread || self.video_thread_running)) {
            self.handle_user_input(emu_cmd_tx);

            for emulator_status in emu_stat_rx.try_iter() {
                self.handle_emulator_status_info(emulator_status, &mut waiting_for_logic_core_thread, &mut waiting_for_video_thread);
            }
            match msg_source.collect_messages() {
                Some(messages) => {
                    for logged_msg in messages {
                        self.emulator_message(logged_msg.as_str());
                    }
                },
                None => { },
            }
            self.update_screen();
            thread::sleep(sleep_len);
        }
    }
    fn handle_emulator_status_info(&mut self, emulator_status: EmulatorStatus, waiting_for_logic_core_thread: &mut bool, waiting_for_video_thread: &mut bool) {

        match emulator_status {
            EmulatorStatus::Created => {
                self.logic_core_thread_running = true;
                *waiting_for_logic_core_thread = false;
                self.emulator_message("Logic core thread started.");
            },
            EmulatorStatus::Destroyed => {
                if !self.exit_request {
                    panic!("Unexpected termination of the logic core thread");
                } else {
                    self.logic_core_thread_running = false;
                }
            },
            EmulatorStatus::TerminateNotification => {
                self.exit_request = true;
            },
            EmulatorStatus::VideoThreadCreated => {
                self.video_thread_running = true;
                *waiting_for_video_thread = false;
                self.emulator_message("SDL2 front-end thread started.");
            },
            EmulatorStatus::VideoThreadDestroyed => {
                if !self.exit_request {
                    panic!("Unexpected termination of the SDL2 front-end thread");
                } else {
                    self.video_thread_running = false;
                }
            },
            EmulatorStatus::PoweredOn => {
                if !self.machine_powered_on {
                    self.machine_powered_on = true;
                    self.redraw_needed = true;
                }
            },
            EmulatorStatus::PoweredOff => {
                if self.machine_powered_on {
                    self.machine_powered_on = false;
                    self.redraw_needed = true;
                }
            },
            EmulatorStatus::Paused => {
                if !self.machine_paused {
                    self.machine_paused = true;
                    self.redraw_needed = true;
                }
            },
            EmulatorStatus::NotPaused => {
                if self.machine_paused {
                    self.machine_paused = false;
                    self.redraw_needed = true;
                }
            },
            EmulatorStatus::CpuHalted => {
                if !self.cpu_halted {
                    self.cpu_halted = true;
                    self.redraw_needed = true;
                }
            },
            EmulatorStatus::CpuNotHalted => {
                if self.cpu_halted {
                    self.cpu_halted = false;
                    self.redraw_needed = true;
                }
            },
        }
    }
    pub fn handle_user_input(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        loop {
            let user_input = self.window.getch();
            match user_input {
                Some(input) => {
                    match input {
                        pancurses::Input::KeyResize     => { self.handle_resize_event() },

                        pancurses::Input::KeyF1         => { self.execute_command(emu_cmd_tx, "help"); },

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

                        pancurses::Input::KeyEnter      => { self.prompt_handle_enter_key(emu_cmd_tx); },

                        pancurses::Input::Unknown(input_code) => {
                            match input_code {
                                155 => { self.prompt_handle_enter_key(emu_cmd_tx); },        // Enter (keypad, w32)
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
                                    0x0D  => { self.prompt_handle_enter_key(emu_cmd_tx); },  // Enter
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
    pub fn execute_command(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, input_str: &str) {

        let command = match util::get_word(input_str, 1) {
                          Some(command_word) => { command_word.to_lowercase() },
                          None => { return },
                      };

        if command == "exit" || command == "quit" {

            emu_cmd_tx.send(EmulatorCommand::Terminate).unwrap();

        } else if command == "nmi" {

            emu_cmd_tx.send(EmulatorCommand::NmiRequest).unwrap();
            self.emulator_message("Issued a NMI request.");

        // Alias for "clear screen":
        } else if command == "clear" || command == "cls" {
            self.execute_command(emu_cmd_tx, "messages clear all")

        // Alias for "pause" and "unpause":
        } else if command == "pause" {
            self.execute_command(emu_cmd_tx, "machine pause on")

        } else if command == "unpause" {
            self.execute_command(emu_cmd_tx, "machine pause off")

        } else {
            self.execute_parsed_command(emu_cmd_tx, ParsedUserCommand::parse(input_str));
        }
    }
    fn execute_parsed_command(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, command: ParsedUserCommand) {
        match command {
            ParsedUserCommand::Help(help_entry) => {
                self.show_help_entry(help_entry);
            },
            ParsedUserCommand::Messages(sub_command) => {
                self.execute_messages_subcommand(sub_command);
            },
            ParsedUserCommand::Machine(sub_command) => {
                self.execute_machine_subcommand(emu_cmd_tx, sub_command);
            },
            ParsedUserCommand::Memory(sub_command) => {
                self.execute_memory_subcommand(emu_cmd_tx, sub_command);
            },
            ParsedUserCommand::Cassette(sub_command) => {
                self.execute_cassette_subcommand(emu_cmd_tx, sub_command);
            },
            ParsedUserCommand::Config(sub_command) => {
                self.execute_config_subcommand(emu_cmd_tx, sub_command);
            },
            ParsedUserCommand::CommandMissingParameter  { sup_command_name, sub_command_name, parameter_desc, parameter_desc_ia } => {
                self.emulator_message(format!("The `{} {}' command requires {} {} parameter, see: /help {}", sup_command_name, sub_command_name, parameter_desc_ia, parameter_desc, sup_command_name).as_str());
            },
            ParsedUserCommand::CommandMissingSubcommand { sup_command_name } => {
                self.emulator_message(format!("The `{}' command requires a sub-command, see: /help {}", sup_command_name, sup_command_name).as_str());
            },
            ParsedUserCommand::InvalidCommand           { command_name } => {
                 self.emulator_message(format!("Unknown command `{}'.  See `/help' with no argument for a list of supported commands.", command_name).as_str());
            },
            ParsedUserCommand::InvalidSubCommand        { sup_command_name, sub_command_name } => {
                self.emulator_message(format!("Invalid sub-command `{}' for the `{}' command, see: /help {}", sub_command_name, sup_command_name, sup_command_name).as_str());
            },
            ParsedUserCommand::InvalidParameter         { sup_command_name, sub_command_name, parameter_text, parameter_desc } => {
                self.emulator_message(format!("Invalid {} parameter `{}' for the `{} {}' command, see: /help {}", parameter_desc, parameter_text, sup_command_name, sub_command_name, sup_command_name).as_str());
            },
        }
    }
    fn show_help_entry(&mut self, help_entry: HelpEntry) {
        match help_entry {
            HelpEntry::Default => {
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
                self.emulator_message("    cassette    - allows you to change the state of the cassette drive.");
                self.emulator_message("    config      - allows you to change configuration settings.");
                self.emulator_message("");
                self.emulator_message("    F1          - alias for `help', pressing F1 shows this message.");
                self.emulator_message("    clear, cls  - aliases for `messages clear all'.");
                self.emulator_message("    pause       - alias for `machine pause on'.");
                self.emulator_message("    unpause     - alias for `machine pause off'.");
                self.emulator_message("");
                self.emulator_message("Type `/help command' for more information about specific commands.");
            },
            HelpEntry::Help => {
                self.emulator_message("The `help' command is used to explain the commands that are available in the curses-based user interface of the emulator.");
                self.emulator_message("");
                self.emulator_message("For a list of commands, type `/help' with no argument.");
                self.emulator_message("");
                self.emulator_message("For more information about a specific command, type `/help command', where `command' is one of the comands returned by `/help'.");
            },
            HelpEntry::Messages => {
                self.emulator_message("The `messages' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    messages show <machine|emulator>      - makes the given type of messages visible.");
                self.emulator_message("    messages hide <machine|emulator>      - makes the given type of messages invisible.");
                self.emulator_message("    messages toggle <machine|emulator>    - toggles the visibility of messages of the given type.");
                self.emulator_message("    messages clear <machine|emulator|all> - clears/removes messages of the given type.");
                self.emulator_message("");
                self.emulator_message("`emulator' messages are ones that are emitted by the emulator itself, `machine' messages are emitted by the emulated machine.");
            },
            HelpEntry::Machine => {
                self.emulator_message("The `machine' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    machine power <on|off>        - powers the machine on or off.");
                self.emulator_message("    machine reset [cpu|full]      - performs a CPU reset, or a full reset.");
                self.emulator_message("    machine restore               - puts the machine into a default state.");
                self.emulator_message("    machine switch-rom <num>      - change the currently used BASIC rom (Level 1 or 2, or 3 for misc rom).");
                self.emulator_message("    machine pause [on|off|toggle] - pauses or unpauses the machine.");
                self.emulator_message("    machine unpause               - alias for `machine pause off'.");
                self.emulator_message("");
                self.emulator_message("With no argument, `machine reset' performs a CPU reset, and `machine pause' pauses the machine's emulation.");
                self.emulator_message("");
                self.emulator_message("The `machine switch-rom' command is used for changing the currently selected system ROM.  Plese note that switching the ROM involves restarting the machine, so any unsaved progress will be lost.  Valid options are 1 for Level 1 BASIC, 2 for Level 2 BASIC, and 3 for the miscellaneous rom.");
                self.emulator_message("");
                self.emulator_message("The `machine restore' command, on the other hand, is useful for when you've been messing around with the `memory load' and `memory wipe' commands, and want to get back to a normal state by restoring the currently selected system ROM.");
            },
            HelpEntry::Memory => {
                self.emulator_message("The `memory' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    memory load <rom|ram> <file> [offset] - loads a file into either ram or rom.");
                self.emulator_message("    memory wipe <rom|ram|all>             - clears the contents of rom, ram, or both.");
                self.emulator_message("");
                self.emulator_message("The offset specifier in `memory load' can be in either decimal, octal, binary or hexadecimal notation.  The default is decimal, a prefix of 0b means binary, 0x means hexadecimal, 0 means octal, and a postfix of h means hexadecimal.");
                self.emulator_message("");
                self.emulator_message("In the current implementation, file names may not contain spaces and non-ascii characters.  Also, if you pass `default' as the filename to `memory load rom', it will load a default rom image from a pre-defined location.");
            },
            HelpEntry::Cassette => {
                self.emulator_message("The `cassette' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    cassette insert <format> <file> - loads a file into the cassette drive.");
                self.emulator_message("    cassette eject                  - removes the currently inserted cassette from the drive.");
                self.emulator_message("    cassette erase                  - clears the contents of the inserted cassette.");
                self.emulator_message("    cassette seek   <position>      - rewinds the tape to the specified location.");
                self.emulator_message("    cassette rewind                 - rewinds the tape to the beginning.");
                self.emulator_message("");
                self.emulator_message("The position argument to `/cassette seek' is a byte offset within the cassette file.  To get the current value of this offset, issue `/config show cassette_file_offset'.");
                self.emulator_message("");
                self.emulator_message("The file argument to the `/cassette load' command can either be a plain file name, which means a file with that name in the configuration directory, or a full path.  If the specified file doesn't exists, it will be created.  The format argument can be either CAS or CPT.");
                self.emulator_message("");
                self.emulator_message("In the current implementation, file names may not contain non-ascii characters, since there is no way to enter such characters in this user interface.");
            },
            HelpEntry::Config => {
                self.emulator_message("The `config' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    list                               - shows all config entries and their current value.");
                self.emulator_message("    show   <section>_<entry>           - shows the value of the given config entry.");
                self.emulator_message("    change <section>_<entry> = <value> - changes the value of the given config entry.");
                self.emulator_message("");
                self.emulator_message("Invoking `config change' causes the configuration file to be updated, as well as applying the change, if possible.");
            },
            HelpEntry::Alias { alias_name, aliased_name, help_entry } => {
                self.emulator_message(format!("The `{}' command is an alias for `{}', see `/help {}' for more information.", alias_name, aliased_name, help_entry).as_str());
            },
            HelpEntry::Exit => {
                self.emulator_message("The `exit' or `quit' command closes the emulator program.");
            },
        }
    }
    fn execute_messages_subcommand(&mut self, sub_command: MessagesSubCommand) {
        match sub_command {
            MessagesSubCommand::Show(arg) => {
                match arg {
                    MessagesSubCommandArgExclusive::Machine => {
                        self.show_machine_messages();
                    },
                    MessagesSubCommandArgExclusive::Emulator => {
                        self.show_emulator_messages();
                    },
                }
            },
            MessagesSubCommand::Hide(arg) => {
                match arg {
                    MessagesSubCommandArgExclusive::Machine => {
                        self.hide_machine_messages();
                    },
                    MessagesSubCommandArgExclusive::Emulator => {
                        self.hide_emulator_messages();
                    },
                }
            },
            MessagesSubCommand::Toggle(arg) => {
                match arg {
                    MessagesSubCommandArgExclusive::Machine => {
                        if self.machine_msg_shown {
                            self.hide_machine_messages();
                        } else {
                            self.show_machine_messages();
                        }
                    },
                    MessagesSubCommandArgExclusive::Emulator => {
                        if self.emulator_msg_shown {
                            self.hide_emulator_messages();
                        } else {
                            self.show_emulator_messages();
                        }
                    },
                }
            },
            MessagesSubCommand::Clear(arg) => {
                match arg {
                    MessagesSubCommandArgInclusive::Machine => {
                        self.clear_machine_messages();
                    },
                    MessagesSubCommandArgInclusive::Emulator => {
                        self.clear_emulator_messages();
                    },
                    MessagesSubCommandArgInclusive::Both => {
                        self.clear_all_messages();
                    },
                }
            },
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
    fn execute_machine_subcommand(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, sub_command: MachineSubCommand) {
        match sub_command {
            MachineSubCommand::Power { new_state } => {
                if new_state == true {
                    self.power_on_machine(emu_cmd_tx);
                } else {
                    self.power_off_machine(emu_cmd_tx);
                }
            },
            MachineSubCommand::Reset { full_reset } => {
                if full_reset == true {
                    self.reset_machine_full(emu_cmd_tx);
                } else {
                    self.reset_machine(emu_cmd_tx);
                }
            },
            MachineSubCommand::Restore => {
                self.restore_machine(emu_cmd_tx);
            },
            MachineSubCommand::SwitchRom(rom_nr) => {
                emu_cmd_tx.send(EmulatorCommand::SwitchRom(rom_nr)).unwrap();
            },
            MachineSubCommand::Pause(pause_type) => {
                match pause_type {
                    PauseType::Pause => {
                        self.pause_machine(emu_cmd_tx);
                    },
                    PauseType::Unpause => {
                        self.unpause_machine(emu_cmd_tx);
                    },
                    PauseType::Toggle => {
                        if self.machine_paused {
                            self.unpause_machine(emu_cmd_tx);
                        } else {
                            self.pause_machine(emu_cmd_tx);
                        }
                    },
                }
            },
        }
    }
    fn power_off_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {

        if !self.machine_powered_on {
            self.emulator_message("The machine is already powered off.");
        } else {
            emu_cmd_tx.send(EmulatorCommand::PowerOff).unwrap();
        }
    }
    fn power_on_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {

        if self.machine_powered_on {
            self.emulator_message("The machine is already powered on.");
        } else {
            emu_cmd_tx.send(EmulatorCommand::PowerOn).unwrap();
        }
    }
    fn restore_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {

        if self.machine_powered_on {
            self.emulator_message("Cannot restore the machine while it's running.");
        } else {
            self.emulator_message("The machine shall be restored back to its factory-original state.");
            emu_cmd_tx.send(EmulatorCommand::Pause).unwrap();
            emu_cmd_tx.send(EmulatorCommand::ResetHard).unwrap();
            emu_cmd_tx.send(EmulatorCommand::PowerOff).unwrap();
            self.execute_command(emu_cmd_tx, "memory load rom default");
            if !self.machine_paused {
                emu_cmd_tx.send(EmulatorCommand::Unpause).unwrap();
            }
        }
    }
    fn reset_machine_full(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        if self.machine_powered_on {
            emu_cmd_tx.send(EmulatorCommand::ResetHard).unwrap();
        } else {
            self.emulator_message("Cannot reset a powered-off machine.");
        }
    }
    fn reset_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        if self.machine_powered_on {
            emu_cmd_tx.send(EmulatorCommand::ResetSoft).unwrap();
        } else {
            self.emulator_message("Cannot reset a powered-off machine.");
        }
    }
    fn pause_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        if self.machine_paused {
            self.emulator_message("The machine emulation is already paused.");
        } else {
            emu_cmd_tx.send(EmulatorCommand::Pause).unwrap();
        }
    }
    fn unpause_machine(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        if !self.machine_paused {
            self.emulator_message("The machine emulation is already not paused.");
        } else {
            emu_cmd_tx.send(EmulatorCommand::Unpause).unwrap();
        }
    }
    fn execute_memory_subcommand(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, sub_command: MemorySubCommand) {
        match sub_command {
            MemorySubCommand::Load { device, path, offset } => {
                match device {
                    MemorySubCommandArgExclusive::RAM => {
                        emu_cmd_tx.send(EmulatorCommand::LoadSystemRam { path: path, offset: offset }).unwrap();
                    },
                    MemorySubCommandArgExclusive::ROM => {
                        if path == ("default".as_ref() as &OsStr) {
                            emu_cmd_tx.send(EmulatorCommand::LoadSystemRomDefault).unwrap();
                        } else {
                            emu_cmd_tx.send(EmulatorCommand::LoadSystemRom { path: path, offset: offset }).unwrap();
                        }
                    },
                }
            },
            MemorySubCommand::Wipe { device } => {
                match device {
                    MemorySubCommandArgInclusive::RAM => {
                        emu_cmd_tx.send(EmulatorCommand::WipeSystemRam).unwrap();
                    },
                    MemorySubCommandArgInclusive::ROM => {
                        emu_cmd_tx.send(EmulatorCommand::WipeSystemRom).unwrap();
                    },
                    MemorySubCommandArgInclusive::Both => {
                        emu_cmd_tx.send(EmulatorCommand::WipeSystemRam).unwrap();
                        emu_cmd_tx.send(EmulatorCommand::WipeSystemRom).unwrap();
                    },
                }
            },
        }
    }
    fn execute_cassette_subcommand(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, sub_command: EmulatorCassetteCommand) {
        emu_cmd_tx.send(EmulatorCommand::CassetteCommand(sub_command)).unwrap();
    }
    fn execute_config_subcommand(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>, sub_command: EmulatorConfigCommand) {
        emu_cmd_tx.send(EmulatorCommand::ConfigCommand(sub_command)).unwrap();
    }
    fn send_to_console(&mut self, _input_str: String) {
        self.emulator_message("Serial console interface not yet implemented.");
    }
    pub fn add_screen_line(&mut self, line_content: String, line_type: ScreenLineType) {
        enum Action {
            SimplyAppend,
            AppendAndSwap,
            AppendToLast,
        }
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
                let last_line_mut = self.screen_lines.front_mut().expect("if it didn't exist, we wouldn't be appending to it");
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
    fn emulator_message(&mut self, line_content: &str) {
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
    fn machine_line_add_char(&mut self, char_to_add: char) {
        self.add_screen_line(format!("{}", char_to_add), ScreenLineType::MachineMessage { complete: false });
    }
    fn machine_line_finalize(&mut self) {
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
    fn prompt_handle_enter_key(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
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

        if (entered_text.len() > 0) && (entered_text.chars().next().expect("somehow there isn't a first character even though entered_text.len() > 0 evaluated to true") == '/') {
            let (_, command_str) = entered_text.as_str().split_at(1);

            if (command_str.len() > 0) && (command_str.chars().next().expect("somehow there isn't a first character even though command_str.len() > 0 evaluated to true") == '/') {
                 self.send_to_console(command_str.to_owned());
            } else {
                self.execute_command(emu_cmd_tx, command_str);
            }
        } else {
            self.send_to_console(entered_text);
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
    pub fn update_screen(&mut self) {

        if self.redraw_needed {

            self.window.erase();

            if self.screen_too_small {
                self.window.mv(0, 0);
                self.window.addstr(format!("Screen too small, minimum size is {} rows, {} cols.", MIN_SCREEN_HEIGHT, MIN_SCREEN_WIDTH));
            } else {
                self.render_lines();
                self.render_status_strips();
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
                    self.window.addstr(to_print);
                    self.window.attroff(pancurses::colorpair::ColorPair(color_pair));

                    break;
                } else {
                    y_pos = new_y_pos;

                    self.window.mv(y_pos, 0);
                    self.window.attron(pancurses::colorpair::ColorPair(color_pair));
                    self.window.addstr(cur_line_text);
                    self.window.attroff(pancurses::colorpair::ColorPair(color_pair));

                    y_pos -= 1;
                    if y_pos < LINES_TOP_OFFSET as i32 {
                        break;
                    }
                }
            }
        }
    }
    fn render_status_strips(&mut self) {

        // Color the strips:
        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
        self.window.mv(TOP_STRIP_TOP_OFFSET as i32, 0);
        self.window.hline(0x20, self.screen_width as i32);

        self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, 0);
        self.window.hline(0x20, self.screen_width as i32);

        // Write in some text:
        self.window.mv(TOP_STRIP_TOP_OFFSET as i32, 1);
        self.window.addstr(format!("{} v{} - z80 emulator", PROGRAM_NAME, PROGRAM_VERSION).as_str());
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));

        self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, 1);

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        self.window.addch('[');
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

        self.window.attron(pancurses::A_BOLD);
        if self.machine_powered_on {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
            self.window.addstr("power on");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
        } else {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_RED));
            self.window.addstr("power off");
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_RED));
        }
        self.window.attroff(pancurses::A_BOLD);

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        self.window.addch(']');
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

        if self.machine_powered_on || self.machine_paused {
            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            self.window.addch(' ');
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));

            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
            self.window.addch('[');
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));

            if self.machine_paused {
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
                self.window.addstr("paused");
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            } else if self.cpu_halted {
                self.window.attron(pancurses::A_BOLD);
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
                self.window.addstr("halted");
                self.window.attroff(pancurses::A_BOLD);
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
            } else {
                self.window.attron(pancurses::A_BOLD);
                self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
                self.window.addstr("running");
                self.window.attroff(pancurses::A_BOLD);
                self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GREEN));
            }

            self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
            self.window.addch(']');
            self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_CYAN));
        }

        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
        if self.lines_added_scrolled_up {
            self.window.mv((self.screen_height - BOTTOM_STRIP_BOTTOM_OFFSET) as i32 - 1, (self.screen_width as i32) - 1 - 10);
            self.window.addstr("-- more --");
        }
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_STRIP_GRAY));
    }
    fn render_prompt(&mut self) {
        self.window.attron(pancurses::colorpair::ColorPair(COLOR_PAIR_PROMPT));

        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, 0);
        self.window.addstr("> ");

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
        self.window.addstr(to_print);

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
}

impl Drop for UserInterface {
    fn drop(&mut self) {
        pancurses::endwin();
    }
}
