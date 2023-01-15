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

use std::collections::VecDeque;
use std::collections::HashMap;

use crate::emulator;
use crate::memory;
use crate::sdl2;
use crate::util::MessageLogging;

// Even though the keyboard only has (at most) 64 keys, because of the way they
// are wired up, it takes up 256 bytes of the address space.
pub const KBD_MEM_SIZE: u16 = 0x0100;

pub struct KeyboardMemory {
    key_matrix: [u8; 8],

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl memory::MemIO for KeyboardMemory {
    fn read_byte(&mut self, addr: u16, _cycle_timestamp: u32) -> u8 {
        if addr < KBD_MEM_SIZE {
            // The lower byte specifies which row of the matrix is selected.
            let specifier: u8 = (addr & 0x00FF) as u8;

            0 | if (specifier & 0x01) != 0 { self.key_matrix[0] } else { 0 }
              | if (specifier & 0x02) != 0 { self.key_matrix[1] } else { 0 }
              | if (specifier & 0x04) != 0 { self.key_matrix[2] } else { 0 }
              | if (specifier & 0x08) != 0 { self.key_matrix[3] } else { 0 }
              | if (specifier & 0x10) != 0 { self.key_matrix[4] } else { 0 }
              | if (specifier & 0x20) != 0 { self.key_matrix[5] } else { 0 }
              | if (specifier & 0x40) != 0 { self.key_matrix[6] } else { 0 }
              | if (specifier & 0x80) != 0 { self.key_matrix[7] } else { 0 }
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for the keyboard", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8, _cycle_timestamp: u32) {
        if addr < KBD_MEM_SIZE {
            self.log_message(format!("Warning: Attempted to write 0x{:02X} to address 0x{:04X} of the keyboard, this is a no-op.", val, addr));
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for the keyboard", addr);
        }
    }
}
impl KeyboardMemory {
    pub fn new(start_addr: u16) -> KeyboardMemory {
        let mut memory = KeyboardMemory {
            key_matrix: [0; 8],

            logged_messages: Vec::new(),
            messages_present: false,
        };

        memory.log_message(format!("Created the keyboard, starting address: 0x{:04X}, spanning {} bytes.", start_addr, KBD_MEM_SIZE));
        memory
    }
}


// The representation of the keyboard actions that get applied to the data bus.
enum KeyboardQueueEntryAction {
    Press,
    Release,
}
struct KeyboardQueueEntry {
    action: KeyboardQueueEntryAction,
    row:    u8,
    column: u8,
}

struct KeyDesc {
    row:    u8,
    column: u8,
}
enum RedundantKeyVariant {
    Left,
    Right,
}
struct RedundantKeyDesc {
    control_index: usize,
    variant:       RedundantKeyVariant,
    row:           u8,
    column:        u8,
}

#[derive(Copy, Clone)]   // Traits needed for initialization.
struct RedundantKeyControl {
    left_key_pressed:  bool,
    right_key_pressed: bool,
}

fn new_key_map() -> HashMap<i32, KeyDesc> {
    use sdl2::keyboard::Scancode;
    let mut map = HashMap::new();

    map.insert(Scancode::LeftBracket as i32,
               KeyDesc { row: 0, column: 0b0000_0001 });
    map.insert(Scancode::A as i32,
               KeyDesc { row: 0, column: 0b0000_0010 });
    map.insert(Scancode::B as i32,
               KeyDesc { row: 0, column: 0b0000_0100 });
    map.insert(Scancode::C as i32,
               KeyDesc { row: 0, column: 0b0000_1000 });
    map.insert(Scancode::D as i32,
               KeyDesc { row: 0, column: 0b0001_0000 });
    map.insert(Scancode::E as i32,
               KeyDesc { row: 0, column: 0b0010_0000 });
    map.insert(Scancode::F as i32,
               KeyDesc { row: 0, column: 0b0100_0000 });
    map.insert(Scancode::G as i32,
               KeyDesc { row: 0, column: 0b1000_0000 });
    map.insert(Scancode::H as i32,
               KeyDesc { row: 1, column: 0b0000_0001 });
    map.insert(Scancode::I as i32,
               KeyDesc { row: 1, column: 0b0000_0010 });
    map.insert(Scancode::J as i32,
               KeyDesc { row: 1, column: 0b0000_0100 });
    map.insert(Scancode::K as i32,
               KeyDesc { row: 1, column: 0b0000_1000 });
    map.insert(Scancode::L as i32,
               KeyDesc { row: 1, column: 0b0001_0000 });
    map.insert(Scancode::M as i32,
               KeyDesc { row: 1, column: 0b0010_0000 });
    map.insert(Scancode::N as i32,
               KeyDesc { row: 1, column: 0b0100_0000 });
    map.insert(Scancode::O as i32,
               KeyDesc { row: 1, column: 0b1000_0000 });
    map.insert(Scancode::P as i32,
               KeyDesc { row: 2, column: 0b0000_0001 });
    map.insert(Scancode::Q as i32,
               KeyDesc { row: 2, column: 0b0000_0010 });
    map.insert(Scancode::R as i32,
               KeyDesc { row: 2, column: 0b0000_0100 });
    map.insert(Scancode::S as i32,
               KeyDesc { row: 2, column: 0b0000_1000 });
    map.insert(Scancode::T as i32,
               KeyDesc { row: 2, column: 0b0001_0000 });
    map.insert(Scancode::U as i32,
               KeyDesc { row: 2, column: 0b0010_0000 });
    map.insert(Scancode::V as i32,
               KeyDesc { row: 2, column: 0b0100_0000 });
    map.insert(Scancode::W as i32,
               KeyDesc { row: 2, column: 0b1000_0000 });
    map.insert(Scancode::X as i32,
               KeyDesc { row: 3, column: 0b0000_0001 });
    map.insert(Scancode::Y as i32,
               KeyDesc { row: 3, column: 0b0000_0010 });
    map.insert(Scancode::Z as i32,
               KeyDesc { row: 3, column: 0b0000_0100 });
    map.insert(Scancode::Minus as i32,
               KeyDesc { row: 5, column: 0b0000_0100 });
    map.insert(Scancode::Semicolon as i32,
               KeyDesc { row: 5, column: 0b0000_1000 });
    map.insert(Scancode::Comma as i32,
               KeyDesc { row: 5, column: 0b0001_0000 });
    map.insert(Scancode::Equals as i32,
               KeyDesc { row: 5, column: 0b0010_0000 });
    map.insert(Scancode::Slash as i32,
               KeyDesc { row: 5, column: 0b1000_0000 });
    map.insert(Scancode::Up as i32,
               KeyDesc { row: 6, column: 0b0000_1000 });
    map.insert(Scancode::Down as i32,
               KeyDesc { row: 6, column: 0b0001_0000 });
    map.insert(Scancode::Right as i32,
               KeyDesc { row: 6, column: 0b0100_0000 });
    map.insert(Scancode::Space as i32,
               KeyDesc { row: 6, column: 0b1000_0000 });

    map
}

fn new_redundant_key_map() -> HashMap<i32, RedundantKeyDesc> {
    use sdl2::keyboard::Scancode;
    let mut map = HashMap::new();

    // I don't know this for certain, but I think that with the number pad
    // present on the TRS-80 Model I, the keys on the number pad cross the
    // same wires as the ones on the main keyboard, acting as redundant keys.
    //
    // If this is not so, please report it as a bug.

    // Number 0:
    map.insert(Scancode::Num0 as i32,
               RedundantKeyDesc {
                   control_index: 0,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0000_0001,
               });
    map.insert(Scancode::Kp0 as i32,
               RedundantKeyDesc {
                   control_index: 0,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0000_0001,
               });

    // Number 1:
    map.insert(Scancode::Num1 as i32,
               RedundantKeyDesc {
                   control_index: 1,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0000_0010,
               });
    map.insert(Scancode::Kp1 as i32,
               RedundantKeyDesc {
                   control_index: 1,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0000_0010,
               });

    // Number 2:
    map.insert(Scancode::Num2 as i32,
               RedundantKeyDesc {
                   control_index: 2,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0000_0100,
               });
    map.insert(Scancode::Kp2 as i32,
               RedundantKeyDesc {
                   control_index: 2,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0000_0100,
               });

    // Number 3:
    map.insert(Scancode::Num3 as i32,
               RedundantKeyDesc {
                   control_index: 3,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0000_1000,
               });
    map.insert(Scancode::Kp3 as i32,
               RedundantKeyDesc {
                   control_index: 3,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0000_1000,
               });

    // Number 4:
    map.insert(Scancode::Num4 as i32,
               RedundantKeyDesc {
                   control_index: 4,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0001_0000,
               });
    map.insert(Scancode::Kp4 as i32,
               RedundantKeyDesc {
                   control_index: 4,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0001_0000,
               });

    // Number 5:
    map.insert(Scancode::Num5 as i32,
               RedundantKeyDesc {
                   control_index: 5,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0010_0000,
               });
    map.insert(Scancode::Kp5 as i32,
               RedundantKeyDesc {
                   control_index: 5,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0010_0000,
               });

    // Number 6:
    map.insert(Scancode::Num6 as i32,
               RedundantKeyDesc {
                   control_index: 6,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b0100_0000,
               });
    map.insert(Scancode::Kp6 as i32,
               RedundantKeyDesc {
                   control_index: 6,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b0100_0000,
               });

    // Number 7:
    map.insert(Scancode::Num7 as i32,
               RedundantKeyDesc {
                   control_index: 7,
                   variant:       RedundantKeyVariant::Left,
                   row:           4,
                   column:        0b1000_0000,
               });
    map.insert(Scancode::Kp7 as i32,
               RedundantKeyDesc {
                   control_index: 7,
                   variant:       RedundantKeyVariant::Right,
                   row:           4,
                   column:        0b1000_0000,
               });

    // Number 8:
    map.insert(Scancode::Num8 as i32,
               RedundantKeyDesc {
                   control_index: 8,
                   variant:       RedundantKeyVariant::Left,
                   row:           5,
                   column:        0b0000_0001,
               });
    map.insert(Scancode::Kp8 as i32,
               RedundantKeyDesc {
                   control_index: 8,
                   variant:       RedundantKeyVariant::Right,
                   row:           5,
                   column:        0b0000_0001,
               });

    // Number 9:
    map.insert(Scancode::Num9 as i32,
               RedundantKeyDesc {
                   control_index: 9,
                   variant:       RedundantKeyVariant::Left,
                   row:           5,
                   column:        0b0000_0010,
               });
    map.insert(Scancode::Kp9 as i32,
               RedundantKeyDesc {
                   control_index: 9,
                   variant:       RedundantKeyVariant::Right,
                   row:           5,
                   column:        0b0000_0010,
               });

    // There are two enter keys, the main one and the one on the number pad.
    map.insert(Scancode::Return as i32,
               RedundantKeyDesc {
                   control_index: 10,
                   variant:       RedundantKeyVariant::Left,
                   row:           6,
                   column:        0b0000_0001,
               });
    map.insert(Scancode::KpEnter as i32,
               RedundantKeyDesc {
                   control_index: 10,
                   variant:       RedundantKeyVariant::Right,
                   row:           6,
                   column:        0b0000_0001,
               });

    // There are two period keys, the main one and the one on the number pad.
    map.insert(Scancode::Period as i32,
               RedundantKeyDesc {
                   control_index: 11,
                   variant:       RedundantKeyVariant::Left,
                   row:           5,
                   column:        0b0100_0000,
               });
    map.insert(Scancode::KpPeriod as i32,
               RedundantKeyDesc {
                   control_index: 11,
                   variant:       RedundantKeyVariant::Right,
                   row:           5,
                   column:        0b0100_0000,
               });

    // The break key is represented by F1 and Insert.
    map.insert(Scancode::F1 as i32,
               RedundantKeyDesc {
                   control_index: 12,
                   variant:       RedundantKeyVariant::Left,
                   row:           6,
                   column:        0b0000_0100,
               });
    map.insert(Scancode::Insert as i32,
               RedundantKeyDesc {
                   control_index: 12,
                   variant:       RedundantKeyVariant::Right,
                   row:           6,
                   column:        0b0000_0100,
               });

    // The clear key is represented by F2 and Delete.
    map.insert(Scancode::F2 as i32,
               RedundantKeyDesc {
                   control_index: 13,
                   variant:       RedundantKeyVariant::Left,
                   row:           6,
                   column:        0b0000_0010,
               });
    map.insert(Scancode::Delete as i32,
               RedundantKeyDesc {
                   control_index: 13,
                   variant:       RedundantKeyVariant::Right,
                   row:           6,
                   column:        0b0000_0010,
               });

    // The left-arrow key is represented by the left arrow and backspace.
    map.insert(Scancode::Backspace as i32,
               RedundantKeyDesc {
                   control_index: 14,
                   variant:       RedundantKeyVariant::Left,
                   row:           6,
                   column:        0b0010_0000,
               });
    map.insert(Scancode::Left as i32,
               RedundantKeyDesc {
                   control_index: 14,
                   variant:       RedundantKeyVariant::Right,
                   row:           6,
                   column:        0b0010_0000,
               });

    // Both shift keys act as a single key in the Model I.
    map.insert(Scancode::LShift as i32,
               RedundantKeyDesc {
                   control_index: 15,
                   variant:       RedundantKeyVariant::Left,
                   row:           7,
                   column:        0b0000_0001,
               });
    map.insert(Scancode::RShift as i32,
               RedundantKeyDesc {
                   control_index: 15,
                   variant:       RedundantKeyVariant::Right,
                   row:           7,
                   column:        0b0000_0001,
               });

    map
}

pub struct Keyboard {
    queue:                   VecDeque<KeyboardQueueEntry>,

    key_map:                 HashMap<i32, KeyDesc>,
    redundant_key_map:       HashMap<i32, RedundantKeyDesc>,
    redundant_key_ctl:       [RedundantKeyControl; 16],

    logged_messages:         Vec<String>,
    messages_present:        bool,
}

// Try to add a keyboard change onto the queue. Ignore failures.
fn add_keyboard_event(queue: &mut VecDeque<KeyboardQueueEntry>,
                      entry: KeyboardQueueEntry) {
    //println!("The keyboard queue size is {}, the capacity is {}.",
    //         queue.len(), queue.capacity());
    if queue.len() < queue.capacity() {
    //    println!("There is enough free space, adding entry.");
        queue.push_back(entry);
    }
}

impl Keyboard {
    pub fn new() -> Keyboard {
        Keyboard {
            queue:                VecDeque::with_capacity(4096),

            key_map:              new_key_map(),
            redundant_key_map:    new_redundant_key_map(),
            redundant_key_ctl:    [RedundantKeyControl {
                                      left_key_pressed: false,
                                      right_key_pressed: false,
                                  }; 16],

            logged_messages:      Vec::new(),
            messages_present:     false,
        }
    }

    // Handle SDL events.
    pub fn handle_events(&mut self, runtime: &mut emulator::Runtime,
                         event_pump: &mut sdl2::EventPump) {

        for event in event_pump.poll_iter() {
            match event {

                // Handle user keyboard key presses.
                sdl2::event::Event::KeyDown { repeat, scancode: scancode_in, .. } => {

                    // Only accept non-repeated key-presses.
                    if !repeat {
                        match scancode_in {
                            Some(scancode) => {

                                match scancode {

                                    // F4 (un)pauses the emulated machine
                                    sdl2::keyboard::Scancode::F4 => {
                                        runtime.pause_desired = !runtime.paused;
                                    },

                                    // F5 reboots the emulated machine
                                    sdl2::keyboard::Scancode::F5 => {
                                        runtime.reset_full_request = true;
                                    },


                                    // F11 toggles the full-screen mode
                                    sdl2::keyboard::Scancode::F11 => {
                                        runtime.fullscreen_desired = !runtime.fullscreen;
                                    },

                                    // General key handling:
                                    _ => {
                                        // Check whether it's a regular key:
                                        match self.key_map.get(&(scancode as i32)) {

                                            // Simply press down supported keys.
                                            Some(entry) => {
                                                add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                    action: KeyboardQueueEntryAction::Press,
                                                    row:    entry.row,
                                                    column: entry.column,
                                                });
                                            },

                                            // Check whether it's a redundant key:
                                            None => {
                                                match self.redundant_key_map.get(&(scancode as i32)) {
                                                    Some(entry) => {
                                                        match entry.variant {
                                                            RedundantKeyVariant::Left => {
                                                                if !self.redundant_key_ctl[entry.control_index].right_key_pressed {
                                                                    add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                                        action: KeyboardQueueEntryAction::Press,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                    });
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].left_key_pressed = true;
                                                            },
                                                            RedundantKeyVariant::Right => {
                                                                if !self.redundant_key_ctl[entry.control_index].left_key_pressed {
                                                                    add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                                        action: KeyboardQueueEntryAction::Press,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                    });
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].right_key_pressed = true;
                                                            },
                                                        }
                                                    }

                                                    // Unsupported keys are simply ignored.
                                                    None => { },
                                                }
                                            },
                                        }
                                    },
                                }
                            },
                            None => { },
                        }
                    }
                },

                // Handle user keyboard key releases.
                sdl2::event::Event::KeyUp { repeat, scancode: scancode_in, .. } => {

                    // Only accept non-repeated key-presses.
                    if !repeat {
                        match scancode_in {
                            Some(scancode) => {
                                // The match's here in case we'd like to add special actions upon key release.
                                match scancode {

                                    // General key handling:
                                    _ => {
                                        // Check whether it's a regular key:
                                        match self.key_map.get(&(scancode as i32)) {

                                            // Simply release supported keys.
                                            Some(entry) => {
                                                add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                    action: KeyboardQueueEntryAction::Release,
                                                    row:    entry.row,
                                                    column: entry.column,
                                                });
                                            },

                                            // Check whether it's a redundant key:
                                            None => {
                                                match self.redundant_key_map.get(&(scancode as i32)) {
                                                    Some(entry) => {
                                                        match entry.variant {
                                                            RedundantKeyVariant::Left => {
                                                                if !self.redundant_key_ctl[entry.control_index].right_key_pressed {
                                                                    add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                                        action: KeyboardQueueEntryAction::Release,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                    });
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].left_key_pressed = false;
                                                            },
                                                            RedundantKeyVariant::Right => {
                                                                if !self.redundant_key_ctl[entry.control_index].left_key_pressed {
                                                                    add_keyboard_event(&mut self.queue, KeyboardQueueEntry {
                                                                        action: KeyboardQueueEntryAction::Release,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                    });
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].right_key_pressed = false;
                                                            },
                                                        }
                                                    }

                                                    // Unsupported keys are simply ignored.
                                                    None => { },
                                                }
                                            },
                                        }
                                    },
                                }
                            },
                            None => { },
                        }
                    }
                },
                sdl2::event::Event::Quit {..} => {
                    runtime.sdl_exit_request = true;
                },
                // Ignore any unrecognized events.
                _ => { },
            }
        }
    }

    // If there are pending changes to the keyboard memory, apply one.
    pub fn update(&mut self, memory_system: &mut memory::MemorySystem) {

        let ref mut kbd_mem = memory_system.kbd_mem;
        match self.queue.pop_front() {
            Some(entry) => {
                match entry.action {
                    KeyboardQueueEntryAction::Press => {
                        kbd_mem.key_matrix[entry.row as usize] |= entry.column;
                    }, 
                    KeyboardQueueEntryAction::Release => {
                        kbd_mem.key_matrix[entry.row as usize] &= !entry.column;
                    }, 
                }
            },
            None => { },
        }
    }
}

impl MessageLogging for KeyboardMemory {
    fn log_message(&mut self, message: String) {
        self.logged_messages.push(message);
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

impl MessageLogging for Keyboard {
    fn log_message(&mut self, message: String) {
        self.logged_messages.push(message);
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

