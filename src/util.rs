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

// The message logging mechanism used in the project is having different
// parts of the code have their own message logging, and having the user
// interface periodically check for logged messages and collecting them.
//
pub trait MessageLogging {
    fn log_message(&mut self, message: String);

    fn messages_available(&self) -> bool;
    fn collect_messages(&mut self) -> Vec<String>;
}

// On startup, it's useful to have messages that both end up in the user
// interface and on the regular start-up screen, in case the program fails
// during startup:
pub struct StartupLogger {
    incomplete_message:  Option<String>,

    logged_messages:     Vec<String>,
    messages_present:    bool,
}

impl StartupLogger {
    pub fn new() -> StartupLogger {
        StartupLogger {
            incomplete_message: None,

            logged_messages:    Vec::new(),
            messages_present:   false,
        }
    }
    pub fn log_incomplete_message(&mut self, message: String) {
        print!("{}", message.as_str());

        let incomplete_message = self.incomplete_message.clone();
        self.incomplete_message = None;

        match incomplete_message {
            Some(incomplete_message_str) => {
                self.incomplete_message = Some(format!("{}{}", incomplete_message_str.as_str(), message.as_str()));
            },
            None => {
                self.incomplete_message = Some(message);
            },
        }
    }
}

impl MessageLogging for StartupLogger {
    fn log_message(&mut self, message: String) {
        println!("{}", message.as_str());

        let incomplete_message = self.incomplete_message.clone();
        self.incomplete_message = None;

        match incomplete_message {
            Some(incomplete_message_str) => {
                self.logged_messages.push(format!("{}{}", incomplete_message_str.as_str(), message.as_str()));
            },
            None => {
                self.logged_messages.push(message);
            },
        }
        self.messages_present = true;
    }
    fn messages_available(&self) -> bool {
        self.messages_present
    }
    fn collect_messages(&mut self) -> Vec<String> {
        let logged_thus_far = self.logged_messages.drain(..).collect();
        self.messages_present = false;

        logged_thus_far
    }
}


// A routine which checks whether the given code is a printable ASCII character.
//
pub fn is_ascii_printable(in_byte: u32) -> bool {
    (in_byte >= 0x20) && (in_byte <= 0x7E)
}

// A routine which maps a printable character to an ascii code.
//
// Codes which are not printable return a question mark.
//
pub fn ascii_to_printable_char(in_byte: u8) -> char {
    match in_byte {
        0x20 => ' ',
        0x21 => '!',
        0x22 => '"',
        0x23 => '#',
        0x24 => '$',
        0x25 => '%',
        0x26 => '&',
        0x27 => '\'',
        0x28 => '(',
        0x29 => ')',
        0x2A => '*',
        0x2B => '+',
        0x2C => ',',
        0x2D => '-',
        0x2E => '.',
        0x2F => '/',
        0x30 => '0',
        0x31 => '1',
        0x32 => '2',
        0x33 => '3',
        0x34 => '4',
        0x35 => '5',
        0x36 => '6',
        0x37 => '7',
        0x38 => '8',
        0x39 => '9',
        0x3A => ':',
        0x3B => ';',
        0x3C => '<',
        0x3D => '=',
        0x3E => '>',
        0x3F => '?',
        0x40 => '@',
        0x41 => 'A',
        0x42 => 'B',
        0x43 => 'C',
        0x44 => 'D',
        0x45 => 'E',
        0x46 => 'F',
        0x47 => 'G',
        0x48 => 'H',
        0x49 => 'I',
        0x4A => 'J',
        0x4B => 'K',
        0x4C => 'L',
        0x4D => 'M',
        0x4E => 'N',
        0x4F => 'O',
        0x50 => 'P',
        0x51 => 'Q',
        0x52 => 'R',
        0x53 => 'S',
        0x54 => 'T',
        0x55 => 'U',
        0x56 => 'V',
        0x57 => 'W',
        0x58 => 'X',
        0x59 => 'Y',
        0x5A => 'Z',
        0x5B => '[',
        0x5C => '\\',
        0x5D => ']',
        0x5E => '^',
        0x5F => '_',
        0x60 => '`',
        0x61 => 'a',
        0x62 => 'b',
        0x63 => 'c',
        0x64 => 'd',
        0x65 => 'e',
        0x66 => 'f',
        0x67 => 'g',
        0x68 => 'h',
        0x69 => 'i',
        0x6A => 'j',
        0x6B => 'k',
        0x6C => 'l',
        0x6D => 'm',
        0x6E => 'n',
        0x6F => 'o',
        0x70 => 'p',
        0x71 => 'q',
        0x72 => 'r',
        0x73 => 's',
        0x74 => 't',
        0x75 => 'u',
        0x76 => 'v',
        0x77 => 'w',
        0x78 => 'x',
        0x79 => 'y',
        0x7A => 'z',
        0x7B => '{',
        0x7C => '|',
        0x7D => '}',
        0x7E => '~',
        _ => '?'
    }
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
