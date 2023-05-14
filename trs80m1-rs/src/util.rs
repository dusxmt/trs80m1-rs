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

use log::{Record, Level, LevelFilter, Metadata};

use std::vec::Vec;
use std::sync::Mutex;

// The message logging mechanism used in the project is having a shared
// message logging buffer that various parts of the code submit messages
// to, which are then collected by a user interface module and displayed
// in a scrollable text buffer.
//
struct MessageLoggerState {
    messages:       Vec<String>,
    stdouterr_echo: bool,
}
pub struct MessageLogger {
    state:  Mutex<MessageLoggerState>,
}

impl MessageLogger {
    pub fn new() -> MessageLogger {
        MessageLogger {
            state: Mutex::new(MessageLoggerState {
                messages:       Vec::new(),
                stdouterr_echo: true,
            }),
        }
    }
    pub fn set_stdouterr_echo(&self, new_val: bool) {
        match self.state.lock() {
            Ok(mut state) => {
                state.stdouterr_echo = new_val;
            },
            Err(error) => {
                panic!("Failed to lock message logger state mutex: {}", error);
            },
        }
    }
    pub fn collect_messages(&self) -> Option<Vec<String>> {
        match self.state.lock() {
            Ok(mut state) => {
                if state.messages.len() > 0 {
                    Some(state.messages.drain(..).collect())
                } else {
                    None
                }
            },
            Err(error) => {
                panic!("Failed to lock message logger state mutex: {}", error);
            },
        }
    }
    pub fn set_logger(&'static self) -> Result<(), log::SetLoggerError> {
        log::set_logger(self)?;
        log::set_max_level(LevelFilter::Info);
        Ok(())
    }
}

impl log::Log for MessageLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {

            let message = if record.level() == Level::Info {
                format!("{}", record.args())
            } else {
                format!("{}: {}", record.level(), record.args())
            };

            match self.state.lock() {
                Ok(mut state) => {
                    if state.stdouterr_echo {
                        if record.level() < Level::Info {
                            eprintln!("{}", message);
                        } else {
                            println!("{}", message);
                        }
                    }
                    state.messages.push(message);
                },
                Err(error) => {
                    eprintln!("{}", message);
                    panic!("Failed to lock message logger state mutex: {}", error);
                },
            }
        }
    }

    fn flush(&self) {}
}

// A routine which returns individual words in a string, where a word is
// defined as a set of non-whitespace characters separated by whitespaces.
//
// The words are indexed from 1.
//
// Returns Some(word) if the given word exists, or None if it does not.
//
pub fn get_word(input: &str, order: usize) -> Option<String> {
    let mut outside_of_word = true;
    let mut current_word    = 0;
    let mut collected_word  = "".to_owned();

    for character in input.chars() {
        if outside_of_word {
            if !character.is_whitespace() {
                current_word += 1;
                outside_of_word = false;
            } else if current_word == order {
                break;
            }
        }
        if !outside_of_word {
            if character.is_whitespace() {
                outside_of_word = true;
            } else if current_word == order {
                collected_word.push(character);
            }
        }
    }
    match collected_word.is_empty() {
        false => { Some(collected_word) },
        true  => { None },
    }
}

// A routine which retrieves the part of the input string that starts with
// the selected word, where a word is defined as a set of non-whitespace
// characters separated by whitespaces.
//
// The words are indexed from 1.
//
// Returns Some(text) if the starting word exists, or None if it does not.
//
pub fn get_starting_at_word(input: &str, order: usize) -> Option<String> {
    let mut outside_of_word = true;
    let mut current_word    = 0;
    let mut collected_text  = "".to_owned();

    for character in input.chars() {
        if outside_of_word {
            if !character.is_whitespace() {
                current_word += 1;
                outside_of_word = false;
            }
        }
        if !outside_of_word {
            if character.is_whitespace() {
                outside_of_word = true;
            }
        }
        if current_word >= order {
            collected_text.push(character);
        }
    }
    match collected_text.is_empty() {
        false => { Some(collected_text.trim().to_owned()) },
        true  => { None },
    }
}

// The following routine parses a 32-bit unsigned number from a string,
// accepting the `0x-', `0-' and `0b-' prefixes for hex, octal and binary, and
// the `-h' postfix for hex.  The `_' is stripped from input.
//
// It either returns Some(number) on success, or None on failure.
pub fn parse_u32_from_str(input_in: &str) -> Option<u32> {
    let mut input = input_in.trim().to_lowercase();
    if input.is_empty() {
        return None;
    }
    let mut base:       u32 = 10;
    let mut skip_start: u32 = 0;


    // Find out which base we'll be parsing in:
    {
        let mut input_chars = input.chars();
        let first_char = input_chars.next().expect(".expect() call: First character retrieval in util::parse_u32_from_str()");
        if first_char == '0' && input.len() > 1 {
            let second_char = input_chars.next().expect(".expect() call: Second character retrieval in util::parse_u32_from_str()");
            match second_char {
                'x' => { base = 16; skip_start = 2 },
                'b' => { base = 2;  skip_start = 2 },
                 _  => { base = 8;  skip_start = 1 },
            }
        }
    }
    if skip_start == 0 {
        let last_letter = input.pop().expect(".expect() call: Last character retrieval in util::parse_u32_from_str()");
        if last_letter == 'h' {
            base = 16
        } else {
            input.push(last_letter);
        }
    }

    let mut accumulator: u32 = 0;
    for letter in input.chars() {
        if skip_start > 0 {
            skip_start -= 1;
            continue;
        }
        // The '_' is accepted as a separator character, and is thus skipped:
        if letter == '_' {
            continue;
        }
        let digit = match letter.to_digit(base) {
            Some(digit) => { digit },
            None => { return None; },
        };
        let shifted_accumulator = accumulator.wrapping_mul(base);
        if shifted_accumulator < accumulator {
            return None;
        }
        accumulator = shifted_accumulator + digit;
    }

    Some(accumulator)
}
