// Copyright (c) 2017, 2018, 2023 Marek Benc <benc.marek.elektro98@proton.me>
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

use log::{info, warn, error};

use std::collections::HashMap;
use std::sync::mpsc;

use crate::emulator;
use trs80m1_rs_core::keyboard;


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

pub struct SdlKeyboard {
    key_map:                 HashMap<i32, KeyDesc>,
    redundant_key_map:       HashMap<i32, RedundantKeyDesc>,
    redundant_key_ctl:       [RedundantKeyControl; 16],
    cycles_per_keypress:     u32,
}

impl SdlKeyboard {
    pub fn new(cycles_per_keypress: u32) -> SdlKeyboard {
        SdlKeyboard {
            key_map:              new_key_map(),
            redundant_key_map:    new_redundant_key_map(),
            redundant_key_ctl:    [RedundantKeyControl {
                                      left_key_pressed: false,
                                      right_key_pressed: false,
                                  }; 16],

            cycles_per_keypress,
        }
    }

    pub fn set_cycles_per_keypress(&mut self, cycles_per_keypress: u32) {
        self.cycles_per_keypress = cycles_per_keypress;
    }

    // Handle SDL events.
    pub fn handle_events(&mut self,
                         emu_cmd_tx:         &mpsc::Sender<emulator::EmulatorCommand>,
                         event_pump:         &mut sdl2::EventPump,
                         fullscreen_toggle:  &mut bool,
                         keycode_tx:         &mpsc::Sender<keyboard::KeyboardQueueEntry>,
                         capture_kbd:        bool) {
        *fullscreen_toggle = false;

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
                                        emu_cmd_tx.send(emulator::EmulatorCommand::TogglePause).unwrap();
                                    },

                                    // F5 reboots the emulated machine
                                    sdl2::keyboard::Scancode::F5 => {
                                        emu_cmd_tx.send(emulator::EmulatorCommand::ResetHard).unwrap();
                                    },

                                    // F11 toggles the full-screen mode
                                    sdl2::keyboard::Scancode::F11 => {
                                        *fullscreen_toggle = true;
                                    },

                                    // General key handling:
                                    _ => { if capture_kbd {

                                        // Check whether it's a regular key:
                                        match self.key_map.get(&(scancode as i32)) {

                                            // Simply press down supported keys.
                                            Some(entry) => {
                                                keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                    action: keyboard::KeyboardQueueEntryAction::Press,
                                                    row:    entry.row,
                                                    column: entry.column,
                                                    delay:  self.cycles_per_keypress,
                                                }).unwrap();
                                            },

                                            // Check whether it's a redundant key:
                                            None => {
                                                match self.redundant_key_map.get(&(scancode as i32)) {
                                                    Some(entry) => {
                                                        match entry.variant {
                                                            RedundantKeyVariant::Left => {
                                                                if !self.redundant_key_ctl[entry.control_index].right_key_pressed {
                                                                    keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                                        action: keyboard::KeyboardQueueEntryAction::Press,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                        delay:  self.cycles_per_keypress,
                                                                    }).unwrap();
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].left_key_pressed = true;
                                                            },
                                                            RedundantKeyVariant::Right => {
                                                                if !self.redundant_key_ctl[entry.control_index].left_key_pressed {
                                                                    keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                                        action: keyboard::KeyboardQueueEntryAction::Press,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                        delay:  self.cycles_per_keypress,
                                                                    }).unwrap();
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
                                    }},
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
                                    _ => { if capture_kbd {

                                        // Check whether it's a regular key:
                                        match self.key_map.get(&(scancode as i32)) {

                                            // Simply release supported keys.
                                            Some(entry) => {
                                                keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                    action: keyboard::KeyboardQueueEntryAction::Release,
                                                    row:    entry.row,
                                                    column: entry.column,
                                                    delay:  self.cycles_per_keypress,
                                                }).unwrap();
                                            },

                                            // Check whether it's a redundant key:
                                            None => {
                                                match self.redundant_key_map.get(&(scancode as i32)) {
                                                    Some(entry) => {
                                                        match entry.variant {
                                                            RedundantKeyVariant::Left => {
                                                                if !self.redundant_key_ctl[entry.control_index].right_key_pressed {
                                                                    keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                                        action: keyboard::KeyboardQueueEntryAction::Release,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                        delay:  self.cycles_per_keypress,
                                                                    }).unwrap();
                                                                }
                                                                self.redundant_key_ctl[entry.control_index].left_key_pressed = false;
                                                            },
                                                            RedundantKeyVariant::Right => {
                                                                if !self.redundant_key_ctl[entry.control_index].left_key_pressed {
                                                                    keycode_tx.send(keyboard::KeyboardQueueEntry {
                                                                        action: keyboard::KeyboardQueueEntryAction::Release,
                                                                        row:    entry.row,
                                                                        column: entry.column,
                                                                        delay:  self.cycles_per_keypress,
                                                                    }).unwrap();
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
                                    }},
                                }
                            },
                            None => { },
                        }
                    }
                },
                sdl2::event::Event::Quit {..} => {
                    emu_cmd_tx.send(emulator::EmulatorCommand::Terminate).unwrap();
                },
                // Ignore any unrecognized events.
                _ => { },
            }
        }
    }
}
