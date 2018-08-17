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
use std::path;

use cassette;
use emulator;
use memory;
use memory::MemoryChipOps;
use proj_config;
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
    Pause (PauseType),
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

enum CassetteSubCommand {
    Insert { format: cassette::Format, file: String },
    Eject,
    Erase,
    Seek   { position: usize },
    Rewind,
}

enum ConfigSubCommand {
    List,
    Show   { entry_specifier: String },
    Change { entry_specifier: String, invocation_text: String },
}

enum ParsedUserCommand {
    Help     (HelpEntry),
    Messages (MessagesSubCommand),
    Machine  (MachineSubCommand),
    Memory   (MemorySubCommand),
    Cassette (CassetteSubCommand),
    Config   (ConfigSubCommand),

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
                          None => { panic!("Command string empty."); },
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
                                ParsedUserCommand::Cassette(CassetteSubCommand::Insert { format: format, file: file })
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
                                ParsedUserCommand::Cassette(CassetteSubCommand::Seek { position: position })
                            },
                            Err(_) => {
                                ParsedUserCommand::InvalidParameter { sup_command_name: command, sub_command_name: sub_command, parameter_text: position_str, parameter_desc: "position".to_owned() }
                            },
                        }
                    } else if sub_command == "eject" {
                        ParsedUserCommand::Cassette(CassetteSubCommand::Eject)
                    } else if sub_command == "erase" {
                        ParsedUserCommand::Cassette(CassetteSubCommand::Erase)
                    } else if sub_command == "rewind" {
                        ParsedUserCommand::Cassette(CassetteSubCommand::Rewind)
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
                        ParsedUserCommand::Config(ConfigSubCommand::List)
                    } else if sub_command == "show" {
                        let entry_specifier = match parameter_1 {
                                                  Some((_, parameter_1_raw)) => { parameter_1_raw },
                                                  None => {
                                                      return ParsedUserCommand::CommandMissingParameter { sup_command_name: command, sub_command_name: sub_command, parameter_desc: "entry specifier".to_owned(), parameter_desc_ia: "an".to_owned() };
                                                  },
                                              };
                        ParsedUserCommand::Config(ConfigSubCommand::Show { entry_specifier: entry_specifier })
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
                        ParsedUserCommand::Config(ConfigSubCommand::Change { entry_specifier: entry_specifier, invocation_text: command_string.to_owned() })
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
    pub fn handle_user_input(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {
        loop {
            let user_input = self.window.getch();
            match user_input {
                Some(input) => {
                    match input {
                        pancurses::Input::KeyResize     => { self.handle_resize_event() },

                        pancurses::Input::KeyF1         => { self.execute_command("help", emulator, memory_system, config_system); },

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

                        pancurses::Input::KeyEnter      => { self.prompt_handle_enter_key(emulator, memory_system, config_system); },

                        pancurses::Input::Unknown(input_code) => {
                            match input_code {
                                155 => { self.prompt_handle_enter_key(emulator, memory_system, config_system); },  // Enter (keypad, w32)
                                _   => { },
                            }
                        },

                        pancurses::Input::Character(input_char) => {
                            if util::is_ascii_printable(input_char as u32) {
                                self.prompt_insert_char(util::ascii_to_printable_char(input_char as u8));
                            } else {
                                match input_char as u8 {
                                    0x08  => { self.prompt_handle_backspace_key(); },                                    // Backspace (w32)
                                    0x0C  => { self.window.clearok(true); self.redraw_needed = true; },                  // CTRL+L
                                    0x0D  => { self.prompt_handle_enter_key(emulator, memory_system, config_system); },  // Enter
                                    0x15  => { self.prompt_handle_ctrl_u(); },                                           // CTRL+U
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
    pub fn execute_command(&mut self, input_str: &str, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {

        let command = match util::get_word(input_str, 1) {
                          Some(command_word) => { command_word.to_lowercase() },
                          None => { return },
                      };

        if command == "exit" || command == "quit" {
            emulator.exit_request = true;

        // Alias for "clear screen":
        } else if command == "clear" || command == "cls" {
            self.execute_command("messages clear all", emulator, memory_system, config_system)

        // Alias for "pause" and "unpause":
        } else if command == "pause" {
            self.execute_command("machine pause on", emulator, memory_system, config_system)

        } else if command == "unpause" {
            self.execute_command("machine pause off", emulator, memory_system, config_system)

        } else {
            self.execute_parsed_command (ParsedUserCommand::parse(input_str), emulator, memory_system, config_system);
        }
    }
    fn execute_parsed_command (&mut self, command: ParsedUserCommand, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {
        match command {
            ParsedUserCommand::Help(help_entry) => {
                self.show_help_entry(help_entry);
            },
            ParsedUserCommand::Messages(sub_command) => {
                self.execute_messages_subcommand(sub_command);
            },
            ParsedUserCommand::Machine(sub_command) => {
                self.execute_machine_subcommand(sub_command, emulator, memory_system);
            },
            ParsedUserCommand::Memory(sub_command) => {
                self.execute_memory_subcommand(sub_command, memory_system);
            },
            ParsedUserCommand::Cassette(sub_command) => {
                self.execute_cassette_subcommand(sub_command, memory_system, config_system);
            },
            ParsedUserCommand::Config(sub_command) => {
                self.execute_config_subcommand(sub_command, emulator, memory_system, config_system);
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
                self.emulator_message("    machine pause [on|off|toggle] - pauses or unpauses the machine.");
                self.emulator_message("    machine unpause               - alias for `machine pause off'.");
                self.emulator_message("");
                self.emulator_message("With no argument, `machine reset' performs a CPU reset, and `machine pause' pauses the machine's emulation.");
            },
            HelpEntry::Memory => {
                self.emulator_message("The `memory' command has the following sub-commands:");
                self.emulator_message("");
                self.emulator_message("    memory load <rom|ram> <file> [offset] - loads a file into either ram or rom.");
                self.emulator_message("    memory wipe <rom|ram|all>             - clears the contents of rom, ram, or both.");
                self.emulator_message("");
                self.emulator_message("The offset specifier in `memory load' can be in either decimal, octal, binary or hexadecimal notation.  The default is decimal, a prefix of 0b means binary, 0x means hexadecimal, 0 means octal, and a postfix of h means hexadecimal.");
                self.emulator_message("");
                self.emulator_message("In the current implementation, file names may not contain spaces and non-ascii characters.");
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
                self.emulator_message("Invoking `config change' causes the configuration file to be updated, as well as applying the change, if possible.  Some configuration changes (namely use of hardware accelerated rendering) may need the emulator to be closed and re-opened for the change to take effect.");
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
    fn execute_machine_subcommand(&mut self, sub_command: MachineSubCommand, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem) {
        match sub_command {
            MachineSubCommand::Power { new_state } => {
                if new_state == true {
                    self.power_on_machine(emulator, memory_system);
                } else {
                    self.power_off_machine(emulator, memory_system);
                }
            },
            MachineSubCommand::Reset { full_reset } => {
                if full_reset == true {
                    self.reset_machine_full(emulator, memory_system);
                } else {
                    self.reset_machine(emulator, memory_system);
                }
            },
            MachineSubCommand::Pause(pause_type) => {
                match pause_type {
                    PauseType::Pause => {
                        self.pause_machine(emulator);
                    },
                    PauseType::Unpause => {
                        self.unpause_machine(emulator);
                    },
                    PauseType::Toggle => {
                        if emulator.paused {
                            self.unpause_machine(emulator);
                        } else {
                            self.pause_machine(emulator);
                        }
                    },
                }
            },
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
    fn execute_memory_subcommand(&mut self, sub_command: MemorySubCommand, memory_system: &mut memory::MemorySystem) {
        match sub_command {
            MemorySubCommand::Load { device, path, offset } => {
                match device {
                    MemorySubCommandArgExclusive::RAM => {
                        memory_system.ram_chip.load_from_file(path, offset);
                    },
                    MemorySubCommandArgExclusive::ROM => {
                        memory_system.rom_chip.load_from_file(path, offset);
                    },
                }
            },
            MemorySubCommand::Wipe { device } => {
                match device {
                    MemorySubCommandArgInclusive::RAM => {
                        memory_system.ram_chip.wipe();
                    },
                    MemorySubCommandArgInclusive::ROM => {
                        memory_system.rom_chip.wipe();
                    },
                    MemorySubCommandArgInclusive::Both => {
                        memory_system.ram_chip.wipe();
                        memory_system.rom_chip.wipe();
                    },
                }
            },
        }
    }
    fn execute_cassette_subcommand(&mut self, sub_command: CassetteSubCommand, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {
        match sub_command {
            CassetteSubCommand::Insert { format, file } => {
                if file.to_lowercase() == "none" {
                    self.emulator_message(format!("A filename of `{}' is not allowed, since the config system would understand it as a lack of a cassette.", file).as_str());
                } else {
                    match config_system.change_config_entry("cassette_file", format!("= {}", file).as_str()) {
                        Err(error) => {
                            self.emulator_message(format!("Failed to set the cassette file in the config system: {}.", error).as_str());
                        },
                        Ok(..) => {
                            memory_system.cas_rec.reload_cassette_file(config_system);
                            match config_system.change_config_entry("cassette_file_format", match format {
                                                                                                cassette::Format::CAS => { "= CAS" },
                                                                                                cassette::Format::CPT => { "= CPT" },
                                                                                            }) {
                                Err(error) => {
                                    self.emulator_message(format!("Failed to set the cassette file format in the config system: {}.", error).as_str());
                                },
                                Ok(..) => {
                                    memory_system.cas_rec.change_cassette_data_format(config_system);
                                    match config_system.change_config_entry("cassette_file_offset", "= 0") {
                                        Err(error) => {
                                            self.emulator_message(format!("Failed to set the cassette file offset in the config system: {}.", error).as_str());
                                        },
                                        Ok(..) => {
                                            memory_system.cas_rec.update_cassette_file_offset(config_system);
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            },
            CassetteSubCommand::Eject => {
                match config_system.config_items.cassette_file {

                    Some(..) => {
                        match config_system.change_config_entry("cassette_file", "= none") {
                            Err(error) => {
                                self.emulator_message(format!("Failed to update the cassette file field in the config system: {}.", error).as_str());
                            },
                            Ok(..) => {
                                memory_system.cas_rec.reload_cassette_file(config_system);
                                self.emulator_message("Cassette ejected.");

                                match config_system.change_config_entry("cassette_file_offset", "= 0") {
                                    Ok(_) => {
                                        memory_system.cas_rec.update_cassette_file_offset(config_system);
                                    },
                                    Err(error) => {
                                        self.emulator_message(format!("Note: Failed to reset the the file offset to 0: {}.", error).as_str());
                                    },
                                }
                            },
                        }
                    },
                    None => {
                        self.emulator_message("The cassette drive is already empty.");
                    },
                }
            },
            CassetteSubCommand::Seek { position } => {
                match config_system.change_config_entry("cassette_file_offset", format!("= {}", position).as_str()) {
                    Err(error) => {
                        self.emulator_message(format!("Failed to set the cassette file offset in the config system: {}.", error).as_str());
                    },
                    Ok(..) => {
                        memory_system.cas_rec.update_cassette_file_offset(config_system);
                        self.emulator_message(format!("Cassette rewound to position {}.", position).as_str());
                    },
                }
            },
            CassetteSubCommand::Rewind => {
                match config_system.change_config_entry("cassette_file_offset", "= 0") {
                    Err(error) => {
                        self.emulator_message(format!("Failed to set the cassette file offset in the config system: {}.", error).as_str());
                    },
                    Ok(..) => {
                        memory_system.cas_rec.update_cassette_file_offset(config_system);
                        self.emulator_message("Cassette rewound back to the beginning.");
                    },
                }
            },
            CassetteSubCommand::Erase => {
                memory_system.cas_rec.erase_cassette();
            },
        }
    }
    fn execute_config_subcommand(&mut self, sub_command: ConfigSubCommand, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {
        match sub_command {
            ConfigSubCommand::List => {
                let config_entries = match config_system.get_config_entry_current_state_all() {
                    Ok(entries) => { entries },
                    Err(error) => {
                        self.emulator_message(format!("Failed to retrieve a listing of config entries: {}.", error).as_str());
                        return;
                    },
                };
                self.emulator_message("Listing of configuration entries:");
                for config_entry in config_entries {
                    self.emulator_message(&config_entry);
                }
            },
            ConfigSubCommand::Show { entry_specifier } => {
                let config_entry = match config_system.get_config_entry_current_state(&entry_specifier) {
                    Ok(entry) => { entry },
                    Err(error) => {
                        self.emulator_message(format!("Failed to retrieve the requested config entry: {}.", error).as_str());
                        return;
                    },
                };
                self.emulator_message(&config_entry);
            },
            ConfigSubCommand::Change { entry_specifier, invocation_text } => {
                match config_system.change_config_entry(&entry_specifier, &invocation_text) {
                    Ok(apply_action) => {
                        match apply_action {
                            proj_config::ConfigChangeApplyAction::RomChange(which) => {
                                if which == memory_system.rom_choice {
                                    let was_running = emulator.powered_on;
                                    if was_running {
                                        emulator.power_off(memory_system);
                                    }
                                    memory_system.rom_chip.wipe();
                                    memory_system.load_system_rom(config_system);
                                    if was_running {
                                        emulator.power_on(memory_system);
                                    }
                                    self.emulator_message("System rom changed.");
                                } else {
                                    self.emulator_message("Configuration updated.");
                                }
                            },
                            proj_config::ConfigChangeApplyAction::ChangeRamSize => {
                                memory_system.ram_chip.change_size(config_system.config_items.general_ram_size as u16);
                                self.emulator_message("Ram size changed.");
                            },
                            proj_config::ConfigChangeApplyAction::UpdateMsPerKeypress => {
                                emulator.scheduler_update = true;
                                self.emulator_message("Miliseconds per keypress setting updated.");
                            },
                            proj_config::ConfigChangeApplyAction::ChangeWindowedResolution => {
                                if !emulator.fullscreen {
                                    emulator.resolution_update = true;
                                    self.emulator_message("Windowed mode resolution changed.");
                                } else {
                                    self.emulator_message("Configuration updated.");
                                }
                            },
                            proj_config::ConfigChangeApplyAction::ChangeFullscreenResolution => {
                                if emulator.fullscreen {
                                    emulator.resolution_update = true;
                                    self.emulator_message("Fullscreen mode resolution changed.");
                                } else {
                                    self.emulator_message("Configuration updated.");
                                }
                            },
                            proj_config::ConfigChangeApplyAction::ChangeColor => {
                                emulator.video_config_update = true;
                                self.emulator_message("Color settings updated.");
                            },
                            proj_config::ConfigChangeApplyAction::ChangeHwAccelUsage => {
                                if config_system.config_items.video_use_hw_accel {
                                    self.emulator_message("Hardware acceleration enabled, although closing and re-opening the emulator is required for this change to take effect.");
                                } else {
                                    self.emulator_message("Hardware acceleration disabled, although closing and re-opening the emulator is required for this change to take effect.");
                                }
                            },
                            proj_config::ConfigChangeApplyAction::ChangeCharacterGenerator => {
                                emulator.video_config_update = true;
                                self.emulator_message("Character generator changed.");
                            },
                            proj_config::ConfigChangeApplyAction::ChangeLowercaseModUsage => {
                                memory_system.vid_mem.update_lowercase_mod(config_system.config_items.video_lowercase_mod);
                                if config_system.config_items.video_lowercase_mod {
                                    self.emulator_message("Lowercase mod enabled.");
                                } else {
                                    self.emulator_message("Lowercase mod disabled.");
                                }
                            },
                            proj_config::ConfigChangeApplyAction::UpdateCassetteFile => {
                                memory_system.cas_rec.reload_cassette_file(config_system);
                                self.emulator_message("Cassette file changed.");
                            },
                            proj_config::ConfigChangeApplyAction::UpdateCassetteFileFormat => {
                                memory_system.cas_rec.change_cassette_data_format(config_system);
                                self.emulator_message("Cassette file data format changed.");
                            },
                            proj_config::ConfigChangeApplyAction::UpdateCassetteFileOffset => {
                                memory_system.cas_rec.update_cassette_file_offset(config_system);
                                self.emulator_message("Cassette file offset changed.");
                            },
                            proj_config::ConfigChangeApplyAction::Nothing => {
                                self.emulator_message("Configuration updated.");
                            },
                            proj_config::ConfigChangeApplyAction::AlreadyUpToDate => {
                                self.emulator_message("Nothing to change.");
                            },
                        }
                    },
                    Err(error) => {
                        self.emulator_message(format!("Failed to perform the requested configuration change: {}.", error).as_str());
                        return;
                    },
                }
            },
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
    fn prompt_handle_enter_key(&mut self, emulator: &mut emulator::Emulator, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem) {
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
                self.execute_command(command_str, emulator, memory_system, config_system);
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
