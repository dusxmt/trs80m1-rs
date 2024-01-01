// Copyright (c) 2018, 2023 Marek Benc <benc.marek.elektro98@proton.me>
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
use unicode_width;

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

#[derive(Clone, PartialEq, Debug)]
pub enum ScreenLineType {
    EmulatorMessage,
    MachineMessage { complete: bool },
}

#[derive(Clone, Debug)]
struct ScreenLine {
    line_type:                 ScreenLineType,
    line_codes:                Vec<char>,
    line_widths:               Vec<u8>,
    cached_screen_width:       usize,
    cached_line_screen_rows:   usize,
    cached_last_row_cols:      usize,
    cached_last_scr_col:       bool,
    cached_last_scr_col_str:   String,
    cached_last_scr_col_pos:   usize,
}

impl ScreenLine {
    fn new(line_type: ScreenLineType, screen_width: usize) -> ScreenLine {
        ScreenLine {
            line_type,
            line_codes:                Vec::new(),
            line_widths:               Vec::new(),
            cached_screen_width:       screen_width,
            cached_line_screen_rows:   1,
            cached_last_row_cols:      0,
            cached_last_scr_col:       false,
            cached_last_scr_col_str:   String::new(),
            cached_last_scr_col_pos:   0,
        }
    }
    fn new_from_str(input_text: &str, line_type: ScreenLineType, screen_width: usize) -> ScreenLine {

        let mut line = ScreenLine::new(line_type, screen_width);
        line.append_str(input_text);

        line
    }
    fn append_char(&mut self, to_add: char) {

        let ch_scr_width = match unicode_width::UnicodeWidthChar::width(to_add) {
            Some(width) => { width   },
            None        => { return; },  // skip control characters
        };
        assert!(ch_scr_width <= 0xFF);

        self.line_codes.push(to_add);
        self.line_widths.push(ch_scr_width as u8);

        if self.cached_screen_width != 0 {

            if self.cached_last_row_cols + ch_scr_width > self.cached_screen_width {

                self.cached_line_screen_rows += 1;
                self.cached_last_row_cols = ch_scr_width;

            } else {
                self.cached_last_row_cols += ch_scr_width;
            }
        }
    }
    fn append_str(&mut self, to_add: &str) {

        for ch in to_add.chars() {
            self.append_char(ch);
        }
    }
    fn set_screen_width(&mut self, screen_width: usize) {

        assert!(screen_width > 0);

        self.cached_screen_width = screen_width;
        self.cached_line_screen_rows = 1;
        self.cached_last_row_cols = 0;

        for ch_scr_width_u8 in self.line_widths.iter() {

            let ch_scr_width = *ch_scr_width_u8 as usize;

            if self.cached_last_row_cols + ch_scr_width > self.cached_screen_width {

                self.cached_line_screen_rows += 1;
                self.cached_last_row_cols = ch_scr_width;

            } else {
                self.cached_last_row_cols += ch_scr_width;
            }
        }
    }
    fn screen_rows(&mut self, screen_width: usize) -> usize {

        if self.cached_screen_width != screen_width {

            self.set_screen_width(screen_width);
        }
        self.cached_line_screen_rows
    }
    fn last_row_cols(&mut self, screen_width: usize) -> usize {

        if self.cached_screen_width != screen_width {

            self.set_screen_width(screen_width);
        }
        self.cached_last_row_cols
    }
    fn prepare_utf8str_for_cols(&mut self, out_buf: &mut String, screen_width: usize, start_col: usize, col_count: usize, linewrap_gaps: bool) -> usize {

        out_buf.clear();
        self.cached_last_scr_col = false;

        let mut cols_to_skip = start_col;
        let mut out_buf_cols = 0;
        let mut row_free_cols = screen_width;
        let mut width_iter = 0;

        'outer_loop: for ch in self.line_codes.iter() {

            let ch_scr_width = self.line_widths[width_iter] as usize;

            let mut gap_cols = if !linewrap_gaps {

                0

            } else if ch_scr_width > row_free_cols {

                let old_free_cols = row_free_cols;
                row_free_cols = screen_width - ch_scr_width;

                old_free_cols

            } else {

                row_free_cols -= ch_scr_width;

                0
            };

            if cols_to_skip > 0 && gap_cols > 0 {

                if gap_cols >= cols_to_skip {

                    gap_cols -= cols_to_skip;
                    cols_to_skip = 0;

                } else {

                    cols_to_skip -= gap_cols;
                    gap_cols = 0;
                }
            }

            if cols_to_skip > 0 {

                assert!(gap_cols == 0);

                let blanks_needed = if cols_to_skip < ch_scr_width { ch_scr_width - cols_to_skip } else { 0 };
                let mut blanks = 0;

                while blanks < blanks_needed {

                    if out_buf_cols < col_count {

                        out_buf.push(' ');
                        out_buf_cols += 1;

                    } else {

                        break 'outer_loop;
                    }

                    blanks += 1;
                }

                cols_to_skip = if cols_to_skip > ch_scr_width { cols_to_skip - ch_scr_width } else { 0 };

            } else if out_buf_cols + gap_cols + ch_scr_width <= col_count {

                if gap_cols == 0 && (start_col + out_buf_cols + ch_scr_width) % screen_width == 0 {

                    if !self.cached_last_scr_col {

                        self.cached_last_scr_col_str.clear();
                        self.cached_last_scr_col_pos = (start_col + out_buf_cols) % screen_width;
                        self.cached_last_scr_col = true;
                    }
                    self.cached_last_scr_col_str.push(*ch);

                } else {

                    self.cached_last_scr_col = false;
                }

                out_buf.push(*ch);
                out_buf_cols += gap_cols + ch_scr_width;

            } else {

                break;
            }

            width_iter += 1;
        }

        assert!(out_buf_cols <= col_count);
        out_buf_cols
    }
}

impl ToString for ScreenLine {

    fn to_string(&self) -> String {

        let mut new_str = "".to_owned();

        for ch in self.line_codes.iter() {
            new_str.push(*ch);
        }
        new_str
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

    redraw_text_area:            bool,
    redraw_status:               bool,
    redraw_prompt:               bool,
    redraw_everything:           bool,

    max_screen_lines:            usize,
    screen_lines:                VecDeque<ScreenLine>,
    cached_screen_total_rows:    usize,
    cached_screen_free_rows:     usize,
    cached_penult_line_exists:   bool,
    cached_last_line_exists:     bool,
    bottom_rows_skip:            usize,
    lines_added_scrolled_up:     bool,
    emulator_msg_shown:          bool,
    machine_msg_shown:           bool,

    prompt_text:                 ScreenLine,
    prompt_curs_code_pos:        usize,  // cursor position at a Rust char
    prompt_curs_cell_pos:        usize,  // cursor position at a screen cell
    prompt_scroll_cells:         usize,

    prompt_history:              VecDeque<ScreenLine>,
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
                                     window,
                                     exit_request:                false,
                                     logic_core_thread_running:   false,
                                     video_thread_running:        false,

                                     screen_width:                0,
                                     screen_height:               0,
                                     screen_too_small:            true,

                                     redraw_text_area:            false,
                                     redraw_status:               false,
                                     redraw_prompt:               false,
                                     redraw_everything:           true,

                                     max_screen_lines:            MAX_SCREEN_LINES,
                                     screen_lines:                VecDeque::with_capacity(MAX_SCREEN_LINES),
                                     cached_screen_total_rows:    0,
                                     cached_screen_free_rows:     0,
                                     cached_penult_line_exists:   false,
                                     cached_last_line_exists:     false,
                                     bottom_rows_skip:            0,
                                     lines_added_scrolled_up:     false,
                                     emulator_msg_shown:          true,
                                     machine_msg_shown:           true,

                                     prompt_text:                 ScreenLine::new(ScreenLineType::EmulatorMessage, 0),
                                     prompt_curs_code_pos:        0,
                                     prompt_curs_cell_pos:        0,
                                     prompt_scroll_cells:         0,

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
                    self.redraw_status = true;
                }
            },
            EmulatorStatus::PoweredOff => {
                if self.machine_powered_on {
                    self.machine_powered_on = false;
                    self.redraw_status = true;
                }
            },
            EmulatorStatus::Paused => {
                if !self.machine_paused {
                    self.machine_paused = true;
                    self.redraw_status = true;
                }
            },
            EmulatorStatus::NotPaused => {
                if self.machine_paused {
                    self.machine_paused = false;
                    self.redraw_status = true;
                }
            },
            EmulatorStatus::CpuHalted => {
                if !self.cpu_halted {
                    self.cpu_halted = true;
                    self.redraw_status = true;
                }
            },
            EmulatorStatus::CpuNotHalted => {
                if self.cpu_halted {
                    self.cpu_halted = false;
                    self.redraw_status = true;
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
                                155 => { self.prompt_handle_enter_key(emu_cmd_tx); },                        // Enter (keypad, w32)
                                _   => { },
                            }
                        },

                        pancurses::Input::Character(input_char) => {
                            if (input_char as u32) < 0x20 {
                                match input_char as u8 {
                                    0x08  => { self.prompt_handle_backspace_key(); },                        // Backspace (w32)
                                    0x0C  => { self.window.clearok(true); self.redraw_everything = true; },  // CTRL+L
                                    0x0D  => { self.prompt_handle_enter_key(emu_cmd_tx); },                  // Enter
                                    0x15  => { self.prompt_handle_ctrl_u(); },                               // CTRL+U
                                    _     => { },
                                }
                            } else {
                                self.prompt_insert_char(input_char);
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
                self.prompt_scroll_cells = 0;
                self.scroll_prompt_if_needed();
            }
            self.redraw_everything = true;
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
            self.bottom_rows_skip = 0;
            self.redraw_text_area = true;
            self.emulator_message("Emulator messages shown.");
        }
    }
    fn hide_emulator_messages(&mut self) {
        if !self.emulator_msg_shown {
            self.emulator_message("Emulator messages already hidden.");
        } else {
            self.emulator_msg_shown = false;
            self.bottom_rows_skip = 0;
            self.redraw_text_area = true;
            self.emulator_message("Emulator messages hidden.");
        }
    }
    fn show_machine_messages(&mut self) {
        if self.machine_msg_shown {
            self.emulator_message("Machine messages already visible.");
        } else {
            self.machine_msg_shown = true;
            self.bottom_rows_skip = 0;
            self.redraw_text_area = true;
            self.emulator_message("Machine messages shown.");
        }
    }
    fn hide_machine_messages(&mut self) {
        if !self.machine_msg_shown {
            self.emulator_message("Machine messages already hidden.");
        } else {
            self.machine_msg_shown = false;
            self.bottom_rows_skip = 0;
            self.redraw_text_area = true;
            self.emulator_message("Machine messages hidden.");
        }
    }
    fn clear_machine_messages(&mut self) {
        let mut new_screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        for line in &self.screen_lines {
            match line.line_type {
                ScreenLineType::MachineMessage {..} => { },
                _ => {
                    new_screen_lines.push_back(line.clone());
                },
            }
        }
        self.screen_lines = new_screen_lines;
        self.bottom_rows_skip = 0;
        self.redraw_text_area = true;
        self.emulator_message("Machine messages cleared.");
    }
    fn clear_emulator_messages(&mut self) {
        let mut new_screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        for line in &self.screen_lines {
            match line.line_type {
                ScreenLineType::EmulatorMessage => { },
                _ => {
                    new_screen_lines.push_back(line.clone());
                },
            }
        }
        self.screen_lines = new_screen_lines;
        self.bottom_rows_skip = 0;
        self.redraw_text_area = true;
        self.emulator_message("Emulator messages cleared.");
    }
    fn clear_all_messages(&mut self) {
        self.screen_lines = VecDeque::with_capacity(self.max_screen_lines);
        self.bottom_rows_skip = 0;
        self.redraw_text_area = true;
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
    // Note: insert_row_pos should be the row for which prev_line_last_col is valid,
    //       i.e. the previous line's last row if already_drawn_rows == 0, or the line's
    //       current last row (before being extended), or -1 (equivalent to the previous line ending just off-screen)
    //
    fn render_insert_line(&mut self, line: &mut ScreenLine, is_last_line: bool, rows_to_add: usize, already_drawn_rows: usize, last_row_already_drawn_cols: usize, insert_row_pos: i32, prev_row_last_col: Option<(usize, String, u8)>) {

        let down_push_lines_to_insert = if self.cached_screen_free_rows > rows_to_add { rows_to_add } else { self.cached_screen_free_rows };
        let up_push_lines_to_insert = if rows_to_add > down_push_lines_to_insert { rows_to_add - down_push_lines_to_insert } else { 0 };

        self.cached_screen_free_rows -= down_push_lines_to_insert;

        let mut line_insert_y_start = insert_row_pos;

        if insert_row_pos > -1 && up_push_lines_to_insert > 0 {
            let mut insert_lines = up_push_lines_to_insert;

            // pdcurses requires that the cursor is in the scroll region before configuring it.
            self.window.mv(insert_row_pos + (LINES_TOP_OFFSET as i32), (self.screen_width - 1) as i32);
            self.window.setscrreg(LINES_TOP_OFFSET as i32, insert_row_pos + (LINES_TOP_OFFSET as i32));
            self.window.scrollok(true);
            let one_col_space = ' ';

            self.window.attron(pancurses::colorpair::ColorPair(0));
            while insert_lines > 0 {
                self.window.addch(one_col_space);
                self.window.mv(insert_row_pos + (LINES_TOP_OFFSET as i32), (self.screen_width - 1) as i32);
                insert_lines -= 1;
                line_insert_y_start -= 1;
            }
            self.window.attroff(pancurses::colorpair::ColorPair(0));
            self.window.scrollok(false);

            if line_insert_y_start > -1 {
                match prev_row_last_col {
                    Some((last_col_start_pos, last_col_str, color_pair)) => {

                        self.window.attron(pancurses::colorpair::ColorPair(color_pair));
                        self.window.mvaddstr(line_insert_y_start + (LINES_TOP_OFFSET as i32), last_col_start_pos as i32, last_col_str);
                        self.window.attroff(pancurses::colorpair::ColorPair(color_pair));
                    },
                    None => {
                    },
                }
            }
        }

        if !is_last_line && down_push_lines_to_insert > 0 {
            let mut insert_lines = down_push_lines_to_insert;

            self.window.mv(line_insert_y_start + 1 + (LINES_TOP_OFFSET as i32), 0);
            while insert_lines > 0 {
                self.window.insertln();
                insert_lines -= 1;
            }

            // Note: the insertln() call corrupts everything below the text area, redraw it.
            self.redraw_status = true;
            self.redraw_prompt = true;
        }

        let rows_scrolled_over = if line_insert_y_start < -1 { (-1 - line_insert_y_start) as usize } else { 0 };

        let color_pair = match line.line_type {
            ScreenLineType::EmulatorMessage => {
                COLOR_PAIR_EMSG
            },
            ScreenLineType::MachineMessage {..} => {
                COLOR_PAIR_MMSG
            },
        };

        let mut out_cols_str = String::new();

        let start_col = if already_drawn_rows > 0 {

            (rows_scrolled_over + already_drawn_rows - 1) * self.screen_width + last_row_already_drawn_cols
        } else {
            rows_scrolled_over * self.screen_width
        };

        let _out_cols_count =
            line.prepare_utf8str_for_cols(&mut out_cols_str,
                                          self.screen_width,
                                          start_col,
                                          (rows_to_add - rows_scrolled_over) * self.screen_width + if already_drawn_rows > 0 { self.screen_width - last_row_already_drawn_cols } else { 0 },
                                          true);

        self.window.attron(pancurses::colorpair::ColorPair(color_pair));
        if already_drawn_rows > 0 && last_row_already_drawn_cols < self.screen_width {
            self.window.mv(line_insert_y_start + (rows_scrolled_over as i32) + (LINES_TOP_OFFSET as i32), last_row_already_drawn_cols as i32);
        } else {
            self.window.mv(line_insert_y_start + (rows_scrolled_over as i32) + (LINES_TOP_OFFSET as i32) + 1, 0);
        }
        self.window.addstr(&out_cols_str);
        self.window.attroff(pancurses::colorpair::ColorPair(color_pair));
    }
    pub fn add_screen_line(&mut self, line_content: &str, line_type: ScreenLineType) {
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
        let msg_shown = match line_type {
            ScreenLineType::EmulatorMessage => {
                self.emulator_msg_shown
            }
            ScreenLineType::MachineMessage {..} => {
                self.machine_msg_shown
            }
        };

        match action {
            Action::SimplyAppend => {
                let mut new_line = ScreenLine::new_from_str(line_content, line_type, self.screen_width);

                if msg_shown && self.cached_screen_total_rows == 0 {

                    // The text area has never been rendered:
                    self.redraw_text_area = true;

                    if self.bottom_rows_skip > 0 {
                        self.bottom_rows_skip += new_line.screen_rows(self.screen_width);
                        self.lines_added_scrolled_up = true;
                    }

                } else if msg_shown {

                    let mut prev_line_last_col = None;
                    let mut prev_line_y_lr = -1;
                    let mut last_line_found = false;

                    for line in self.screen_lines.iter() {
                        let (line_visible, color_pair) = match line.line_type {
                            ScreenLineType::EmulatorMessage => {
                                (self.emulator_msg_shown, COLOR_PAIR_EMSG)
                            },
                            ScreenLineType::MachineMessage {..} => {
                                (self.machine_msg_shown, COLOR_PAIR_MMSG)
                            },
                        };
                        if line_visible {
                            prev_line_last_col =
                                if line.cached_last_scr_col {
                                    Some((line.cached_last_scr_col_pos, line.cached_last_scr_col_str.to_owned(), color_pair))
                                } else {
                                    None
                                };
                            prev_line_y_lr =
                                if self.cached_screen_free_rows > 0 {
                                    (self.cached_screen_total_rows - self.cached_screen_free_rows - 1) as i32
                                } else {
                                    (self.cached_screen_total_rows + self.bottom_rows_skip - 1) as i32
                                };
                            last_line_found = true;
                            break;
                        }
                    }
                    let prev_line_last_col = prev_line_last_col;
                    let prev_line_y_lr = prev_line_y_lr;
                    let last_line_found = last_line_found;
                    let line_rows = new_line.screen_rows(self.screen_width);

                    if self.cached_screen_free_rows == 0 && prev_line_y_lr >= (self.cached_screen_total_rows as i32) {
                        self.bottom_rows_skip += line_rows;
                        self.lines_added_scrolled_up = true;
                        self.redraw_status = true;
                    }

                    // If the last line doesn't exist anymore, it's been truncated by
                    // the self.screen_lines.truncate(self.max_screen_lines - 1) call.
                    if self.cached_last_line_exists && !last_line_found
                    {
                        self.redraw_text_area = true;
                    }
                    else if !self.redraw_text_area && !self.redraw_everything && prev_line_y_lr >= -1 && prev_line_y_lr < (self.cached_screen_total_rows as i32) {
                        self.render_insert_line(&mut new_line, true, line_rows, 0, 0, prev_line_y_lr, prev_line_last_col);
                    }
                    self.cached_last_line_exists = true;
                    self.cached_penult_line_exists = last_line_found;
                }

                self.screen_lines.truncate(self.max_screen_lines - 1);
                self.screen_lines.push_front(new_line);
            },
            Action::AppendAndSwap => {
                let mut new_line = ScreenLine::new_from_str(line_content, line_type, self.screen_width);

                if msg_shown && self.cached_screen_total_rows == 0 {

                    // The text area has never been rendered:
                    self.redraw_text_area = true;

                    if self.bottom_rows_skip > 0 {
                        self.bottom_rows_skip += new_line.screen_rows(self.screen_width);
                        self.lines_added_scrolled_up = true;
                    }

                } else if msg_shown {

                    let mut prev_line_last_col = None;
                    let mut last_line_found = false;
                    let mut last_line_rows = 0;
                    let mut penult_line_found = false;

                    for line in self.screen_lines.iter_mut() {
                        let (line_visible, color_pair) = match line.line_type {
                            ScreenLineType::EmulatorMessage => {
                                (self.emulator_msg_shown, COLOR_PAIR_EMSG)
                            },
                            ScreenLineType::MachineMessage {..} => {
                                (self.machine_msg_shown, COLOR_PAIR_MMSG)
                            },
                        };
                        if !last_line_found {

                            last_line_rows = line.screen_rows(self.screen_width);
                            last_line_found = true;

                        } else if line_visible {

                            prev_line_last_col =
                                if line.cached_last_scr_col {
                                    Some((line.cached_last_scr_col_pos, line.cached_last_scr_col_str.to_owned(), color_pair))
                                } else {
                                    None
                                };
                            penult_line_found = true;
                            break;
                        }
                    }

                    let prev_line_y_lr =
                        if self.cached_screen_free_rows > 0 {
                            (self.cached_screen_total_rows - self.cached_screen_free_rows - last_line_rows - 1) as i32
                        } else {
                            (self.cached_screen_total_rows + self.bottom_rows_skip - last_line_rows - 1) as i32
                        };

                    let prev_line_last_col = prev_line_last_col;
                    let prev_line_y_lr = prev_line_y_lr;
                    let last_line_found = last_line_found;
                    let penult_line_found = penult_line_found;
                    let line_rows = new_line.screen_rows(self.screen_width);

                    if self.cached_screen_free_rows == 0 && prev_line_y_lr >= (self.cached_screen_total_rows as i32) {
                        self.bottom_rows_skip += line_rows;
                        self.lines_added_scrolled_up = true;
                        self.redraw_status = true;
                    }

                    // If the penultimate or last line doesn't exist anymore, it's been truncated by
                    // the self.screen_lines.truncate(self.max_screen_lines - 1) call.
                    if (self.cached_penult_line_exists && !penult_line_found) || (self.cached_last_line_exists && !last_line_found) {
                        self.redraw_text_area = true;
                    } else if !self.redraw_text_area && !self.redraw_everything && prev_line_y_lr >= -1 && prev_line_y_lr < (self.cached_screen_total_rows as i32) {
                        self.render_insert_line(&mut new_line, !last_line_found, line_rows, 0, 0, prev_line_y_lr, prev_line_last_col);
                    }

                    if !last_line_found {
                        self.cached_last_line_exists = true;
                    } else {
                        self.cached_penult_line_exists = true;
                    }
                }

                self.screen_lines.truncate(self.max_screen_lines - 1);
                self.screen_lines.push_front(new_line);
                self.screen_lines.swap(0, 1);
            },
            Action::AppendToLast => {
                // Temporarily pop it off of the deque to satisfy the borrow checker.
                let mut last_line = self.screen_lines.pop_front().expect("if it didn't exist, we wouldn't be appending to it");
                let old_line_rows = last_line.screen_rows(self.screen_width);
                let old_line_last_row_cols = last_line.last_row_cols(self.screen_width);

                let line_old_last_col =
                    if msg_shown {
                        let color_pair = match last_line.line_type {
                            ScreenLineType::EmulatorMessage => {
                                COLOR_PAIR_EMSG
                            },
                            ScreenLineType::MachineMessage {..} => {
                                COLOR_PAIR_MMSG
                            },
                        };
                        if last_line.cached_last_scr_col {
                            Some((last_line.cached_last_scr_col_pos, last_line.cached_last_scr_col_str.to_owned(), color_pair))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                last_line.append_str(line_content);
                last_line.line_type = line_type;

                let new_line_rows = last_line.screen_rows(self.screen_width);
                let screen_rows_added = new_line_rows - old_line_rows;

                if msg_shown && self.cached_screen_total_rows == 0 {

                    // The text area has never been rendered:
                    self.redraw_text_area = true;

                    if (screen_rows_added > 0) && (self.bottom_rows_skip > 0) {
                        self.bottom_rows_skip += screen_rows_added;
                        self.lines_added_scrolled_up = true;
                    }

                } else if msg_shown {

                    // Added a modifier character?  Redraw the entire character cell.
                    let old_line_last_row_cols = if old_line_rows == new_line_rows && old_line_last_row_cols > 0 && old_line_last_row_cols == last_line.last_row_cols(self.screen_width) {
                        old_line_last_row_cols - 1
                    } else {
                        old_line_last_row_cols
                    };

                    let this_line_y_lr =
                        if self.cached_screen_free_rows > 0 {
                            (self.cached_screen_total_rows - self.cached_screen_free_rows - 1) as i32
                        } else {
                            (self.cached_screen_total_rows + self.bottom_rows_skip - 1) as i32
                        };

                    if self.cached_screen_free_rows == 0 && this_line_y_lr >= (self.cached_screen_total_rows as i32) {
                        self.bottom_rows_skip += screen_rows_added;
                        self.lines_added_scrolled_up = true;
                        self.redraw_status = true;
                    }

                    if !self.redraw_text_area && !self.redraw_everything && this_line_y_lr >= 0 && this_line_y_lr < (self.cached_screen_total_rows as i32) {
                        self.render_insert_line(&mut last_line, true, screen_rows_added, old_line_rows, old_line_last_row_cols, this_line_y_lr, line_old_last_col);
                    }
                }
                // Return the line back to the deque.
                self.screen_lines.push_front(last_line);
            },
        }
    }
    fn emulator_message(&mut self, line_content: &str) {
        self.add_screen_line(line_content, ScreenLineType::EmulatorMessage);
    }
    fn machine_line_add_char(&mut self, char_to_add: char) {
        self.add_screen_line(format!("{}", char_to_add).as_str(), ScreenLineType::MachineMessage { complete: false });
    }
    fn machine_line_finalize(&mut self) {
        self.add_screen_line("", ScreenLineType::MachineMessage { complete: true });
    }
    fn scroll_lines_up(&mut self) {
        // The user can request to scroll as much as they wish, the lines rendering
        // routine will then normalize this value.
        self.bottom_rows_skip += self.screen_height / 2;
        self.redraw_text_area = true;
    }
    fn scroll_lines_down(&mut self) {
        if self.bottom_rows_skip >= (self.screen_height / 2) {
            self.bottom_rows_skip -= self.screen_height / 2;
        } else {
            self.bottom_rows_skip = 0;
        }

        if self.bottom_rows_skip == 0 && self.lines_added_scrolled_up {
            self.lines_added_scrolled_up = false;
            self.redraw_status = true;
        }

        self.redraw_text_area = true;
    }
    // Sanitize the self.prompt_scroll_cells value so that it isn't inside
    // of a wide character.
    fn sanitize_prompt_scroll(&mut self) {

        let suggested_cell_scroll = self.prompt_scroll_cells;
        let mut found_cells = 0;

        let current_prompt_text = match self.prompt_history_pos {
            0 => { &self.prompt_text },
            _ => { &self.prompt_history[self.prompt_history_pos - 1] },
        };

        for ch_scr_width_u8 in current_prompt_text.line_widths.iter() {

            let ch_scr_width = *ch_scr_width_u8 as usize;

            found_cells += ch_scr_width;

            if found_cells >= suggested_cell_scroll {
                break;
            }
        }
        self.prompt_scroll_cells = found_cells;
    }
    // Calculate self.prompt_curs_cell_pos based on self.prompt_curs_codes_pos
    fn calc_prompt_curs_cell_pos(&mut self) {

        let mut found_cells = 0;
        let mut found_codes = 0;

        let current_prompt_text = match self.prompt_history_pos {
            0 => { &self.prompt_text },
            _ => { &self.prompt_history[self.prompt_history_pos - 1] },
        };

        for ch_scr_width_u8 in current_prompt_text.line_widths.iter() {

            let ch_scr_width = *ch_scr_width_u8 as usize;

            found_codes += 1;
            found_cells += ch_scr_width;

            if found_codes == self.prompt_curs_code_pos {
                break;
            }
        }
        self.prompt_curs_cell_pos = found_cells;
    }
    fn scroll_prompt_if_needed(&mut self) {
        // If the cursor is outside of the visible range of the prompt, we need
        // to scroll it.
        //
        if (self.prompt_curs_cell_pos + PROMPT_TEXT_OFFSET >= self.screen_width + self.prompt_scroll_cells) ||
           (self.prompt_curs_cell_pos < self.prompt_scroll_cells) {

            self.prompt_scroll_cells = 0;
            while self.prompt_curs_cell_pos + PROMPT_TEXT_OFFSET - self.prompt_scroll_cells >= self.screen_width {
                self.prompt_scroll_cells += self.screen_width / 2;
                self.sanitize_prompt_scroll();
            }
            self.redraw_prompt = true;
        }
    }
    fn prompt_insert_char(&mut self, ch: char) {

        let ch_scr_width = match unicode_width::UnicodeWidthChar::width(ch) {
            Some(width) => { width   },
            None        => { return; },  // skip control characters
        };
        assert!(ch_scr_width <= 0xFF);

        if self.prompt_history_pos > 0 {
            self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
            self.prompt_history_pos = 0;
        }
        self.prompt_text.line_codes.insert(self.prompt_curs_code_pos, ch);
        self.prompt_text.line_widths.insert(self.prompt_curs_code_pos, ch_scr_width as u8);
        self.redraw_prompt = true;

        self.prompt_curs_code_pos += 1;
        self.prompt_curs_cell_pos += ch_scr_width;
        self.scroll_prompt_if_needed();
    }
    fn prompt_move_cursor_right(&mut self) {
        let current_prompt_text = match self.prompt_history_pos {
            0 => { &self.prompt_text },
            _ => { &self.prompt_history[self.prompt_history_pos - 1] },
        };
        let prompt_text_len = current_prompt_text.line_codes.len();

        if self.prompt_curs_code_pos < prompt_text_len {
            let ch_scr_width = current_prompt_text.line_widths[self.prompt_curs_code_pos] as usize;

            self.prompt_curs_code_pos += 1;
            self.prompt_curs_cell_pos += ch_scr_width;
            self.scroll_prompt_if_needed();
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_left(&mut self) {
        let current_prompt_text = match self.prompt_history_pos {
            0 => { &self.prompt_text },
            _ => { &self.prompt_history[self.prompt_history_pos - 1] },
        };
        let prompt_text_len = current_prompt_text.line_codes.len();

        if self.prompt_curs_code_pos > 0 {

            self.prompt_curs_code_pos -= 1;
            if self.prompt_curs_code_pos >= prompt_text_len {

                self.prompt_curs_code_pos = prompt_text_len;
                self.calc_prompt_curs_cell_pos();

            } else {

                let ch_scr_width = current_prompt_text.line_widths[self.prompt_curs_code_pos] as usize;
                self.prompt_curs_cell_pos -= ch_scr_width;
            }
            self.scroll_prompt_if_needed();
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_up(&mut self) {
        if self.prompt_history_pos < self.prompt_history.len() {
            self.prompt_history_pos += 1;
            self.prompt_curs_code_pos = match self.prompt_history_pos {
                0 => { self.prompt_text.line_codes.len() },
                _ => { self.prompt_history[self.prompt_history_pos - 1].line_codes.len() },
            };
            self.calc_prompt_curs_cell_pos();
            self.scroll_prompt_if_needed();
            self.redraw_prompt = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_move_cursor_down(&mut self) {
        if self.prompt_history_pos > 0 {
            self.prompt_history_pos -= 1;
            self.prompt_curs_code_pos = match self.prompt_history_pos {
                0 => { self.prompt_text.line_codes.len() },
                _ => { self.prompt_history[self.prompt_history_pos - 1].line_codes.len() },
            };
            self.calc_prompt_curs_cell_pos();
            self.scroll_prompt_if_needed();
            self.redraw_prompt = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_backspace_key(&mut self) {
        if self.prompt_curs_code_pos > 0 {

            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }
            let ch_scr_width = self.prompt_text.line_widths[self.prompt_curs_code_pos - 1] as usize;

            self.prompt_text.line_codes.remove(self.prompt_curs_code_pos - 1);
            self.prompt_text.line_widths.remove(self.prompt_curs_code_pos - 1);
            self.prompt_curs_code_pos -= 1;
            self.prompt_curs_cell_pos -= ch_scr_width;
            self.scroll_prompt_if_needed();
            self.redraw_prompt = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_delete_key(&mut self) {
        let prompt_text_len = match self.prompt_history_pos {
            0 => { self.prompt_text.line_codes.len() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].line_codes.len() },
        };
        if self.prompt_curs_code_pos < prompt_text_len {
            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }
            self.prompt_text.line_codes.remove(self.prompt_curs_code_pos);
            self.prompt_text.line_widths.remove(self.prompt_curs_code_pos);
            self.redraw_prompt = true;
        } else {
            pancurses::beep();
        }
    }
    fn prompt_handle_ctrl_u(&mut self) {
        if self.prompt_curs_code_pos > 0 {
            if self.prompt_history_pos > 0 {
                self.prompt_text = self.prompt_history[self.prompt_history_pos - 1].clone();
                self.prompt_history_pos = 0;
            }

            let new_codes  = self.prompt_text.line_codes.split_off(self.prompt_curs_code_pos);
            let new_widths = self.prompt_text.line_widths.split_off(self.prompt_curs_code_pos);

            self.prompt_text.line_codes = new_codes;
            self.prompt_text.line_widths = new_widths;

            self.prompt_curs_code_pos = 0;
            self.prompt_curs_cell_pos = 0;

            self.scroll_prompt_if_needed();
            self.redraw_prompt = true;
        }
    }
    fn prompt_handle_home_key(&mut self) {
        if self.prompt_curs_code_pos != 0 {
            self.prompt_curs_code_pos = 0;
            self.prompt_curs_cell_pos = 0;
            self.scroll_prompt_if_needed();
        }
    }
    fn prompt_handle_end_key(&mut self) {
        let prompt_text_len = match self.prompt_history_pos {
            0 => { self.prompt_text.line_codes.len() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].line_codes.len() },
        };
        if self.prompt_curs_code_pos != prompt_text_len {
            self.prompt_curs_code_pos = prompt_text_len;
            self.calc_prompt_curs_cell_pos();
            self.scroll_prompt_if_needed();
        }
    }
    fn prompt_handle_enter_key(&mut self, emu_cmd_tx: &mpsc::Sender<EmulatorCommand>) {
        let entered_text_line = match self.prompt_history_pos {
            0 => { self.prompt_text.clone() },
            _ => { self.prompt_history[self.prompt_history_pos - 1].clone() },
        };
        let entered_text = entered_text_line.to_string();

        self.prompt_add_to_history(&entered_text_line);
        self.prompt_text = ScreenLine::new(ScreenLineType::EmulatorMessage, 0);
        self.prompt_curs_code_pos = 0;
        self.prompt_curs_cell_pos = 0;
        self.prompt_history_pos = 0;
        self.scroll_prompt_if_needed();
        self.redraw_prompt = true;

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
    fn prompt_add_to_history(&mut self, to_add: &ScreenLine) {
        // Is this line identical to the last one in history?  If yes, ignore.
        match self.prompt_history.front() {
            Some(last_line) => {
                if to_add.line_codes == last_line.line_codes {
                    return;
                }
            },
            None => { },
        }

        self.prompt_history.truncate(self.prompt_history_max_entries - 1);
        self.prompt_history.push_front(to_add.clone());
    }
    pub fn update_screen(&mut self) {

        if self.redraw_everything {

            self.window.erase();

            if self.screen_too_small {
                self.window.mv(0, 0);
                self.window.addstr(format!("Screen too small, minimum size is {} rows, {} cols.", MIN_SCREEN_HEIGHT, MIN_SCREEN_WIDTH));
            } else {
                self.render_lines(false);
                self.render_status_strips();
                self.render_prompt();
            }

            self.redraw_text_area = false;
            self.redraw_status = false;
            self.redraw_prompt = false;
            self.redraw_everything = false;

        } else {

            if self.redraw_text_area {

                self.render_lines(true);

                self.redraw_text_area = false;
            }

            if self.redraw_status {
                self.render_status_strips();
                self.redraw_status = false;
            }
            if self.redraw_prompt {
                self.render_prompt();
                self.redraw_prompt = false;
            }
        }

        self.set_cursor_pos();
        self.window.refresh();
    }
    // Description:
    //
    // The following routine draws the text area, the "lines", of the console
    // window.  It draws them from bottom to top.
    //
    fn render_lines(&mut self, clear_area: bool) {
        let avail_screen_rows = self.screen_height - LINES_BOTTOM_OFFSET - LINES_TOP_OFFSET;
        let mut screen_rows_to_draw = 0;
        let mut screen_rows_to_scroll_over = 0;

        self.cached_penult_line_exists = false;
        self.cached_last_line_exists = false;

        if clear_area {
            let hline_length = self.screen_width as i32;
            self.window.attron(pancurses::colorpair::ColorPair(0));
            for row in 0..=(avail_screen_rows-1) {
                self.window.mv((row + LINES_TOP_OFFSET) as i32, 0);
                self.window.hline(0x20 /*'+'*/, hline_length);
            }
            self.window.attroff(pancurses::colorpair::ColorPair(0));
        }

        for line in self.screen_lines.iter_mut() {
            // Skip lines which aren't to be shown:
            match line.line_type {
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

            let cur_line_screen_rows = line.screen_rows(self.screen_width);

            if screen_rows_to_draw < avail_screen_rows {
                screen_rows_to_draw += cur_line_screen_rows;
            } else if screen_rows_to_scroll_over < self.bottom_rows_skip {
                if screen_rows_to_draw > avail_screen_rows {
                    screen_rows_to_scroll_over = screen_rows_to_draw - avail_screen_rows;
                    screen_rows_to_draw -= screen_rows_to_scroll_over;
                }
                screen_rows_to_scroll_over += cur_line_screen_rows;
            } else {
                break;
            }
        }

        self.cached_screen_total_rows = avail_screen_rows;
        self.cached_screen_free_rows  = if screen_rows_to_draw < avail_screen_rows { avail_screen_rows - screen_rows_to_draw } else { 0 };

        // Sanitize the bottom_rows_skip variable:
        if self.bottom_rows_skip > screen_rows_to_scroll_over {
            self.bottom_rows_skip = screen_rows_to_scroll_over;
        }

        if screen_rows_to_draw > 0 {
            let mut y_pos = (avail_screen_rows as i32) - 1 + (LINES_TOP_OFFSET as i32);
            if avail_screen_rows > screen_rows_to_draw {
                y_pos -= (avail_screen_rows as i32) - (screen_rows_to_draw as i32);
            }

            for line in self.screen_lines.iter_mut() {
                // Skip lines which aren't to be shown:
                match line.line_type {
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
                if !self.cached_last_line_exists {
                    self.cached_last_line_exists = true;
                } else {
                    self.cached_penult_line_exists = true;
                }
                let mut cur_line_screen_rows_print = line.screen_rows(self.screen_width);

                if screen_rows_to_scroll_over >= cur_line_screen_rows_print {
                    screen_rows_to_scroll_over -= cur_line_screen_rows_print;
                    continue;

                } else if screen_rows_to_scroll_over > 0 {

                    cur_line_screen_rows_print -= screen_rows_to_scroll_over;
                    screen_rows_to_scroll_over = 0;
                }
                let cur_line_screen_rows_print = cur_line_screen_rows_print;

                let color_pair = match line.line_type {
                    ScreenLineType::EmulatorMessage     => { COLOR_PAIR_EMSG },
                    ScreenLineType::MachineMessage {..} => { COLOR_PAIR_MMSG },
                };


                let new_y_pos = y_pos - (cur_line_screen_rows_print as i32) + 1;

                let screen_rows_to_skip = if new_y_pos < LINES_TOP_OFFSET as i32 {
                    ((LINES_TOP_OFFSET as i32) - new_y_pos) as usize
                } else {
                    0
                };

                self.window.mv(new_y_pos + screen_rows_to_skip as i32, 0);

                let mut out_cols_str = String::new();
                let _out_cols_str_cols = line.prepare_utf8str_for_cols(&mut out_cols_str, self.screen_width, screen_rows_to_skip * self.screen_width, (cur_line_screen_rows_print - screen_rows_to_skip) * self.screen_width, true);
                let out_cols_str = out_cols_str;

                self.window.attron(pancurses::colorpair::ColorPair(color_pair));
                self.window.addstr(out_cols_str);
                self.window.attroff(pancurses::colorpair::ColorPair(color_pair));

                y_pos = new_y_pos - 1;
                if y_pos < LINES_TOP_OFFSET as i32 {
                    break;
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
        self.window.addstr(format!("{} v{} - TRS-80 Model I emulator", PROGRAM_NAME, PROGRAM_VERSION).as_str());
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
        self.window.hline(0x20, self.screen_width as i32);
        self.window.addstr("> ");

        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, PROMPT_TEXT_OFFSET as i32);
        let current_prompt_text = match self.prompt_history_pos {
            0 => { &mut self.prompt_text },
            _ => { &mut self.prompt_history[self.prompt_history_pos - 1] },
        };

        let mut out_cols_str = String::new();
        let _out_cols_str_cols = current_prompt_text.prepare_utf8str_for_cols(&mut out_cols_str, self.screen_width, self.prompt_scroll_cells, self.screen_width - PROMPT_TEXT_OFFSET, false);
        let out_cols_str = out_cols_str;

        self.window.addstr(out_cols_str);
        self.window.attroff(pancurses::colorpair::ColorPair(COLOR_PAIR_PROMPT));
    }
    fn set_cursor_pos(&self) {
        self.window.mv((self.screen_height - PROMPT_BOTTOM_OFFSET) as i32 - 1, (self.prompt_curs_cell_pos + PROMPT_TEXT_OFFSET - self.prompt_scroll_cells) as i32);
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
