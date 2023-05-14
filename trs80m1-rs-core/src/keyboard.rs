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

use log::{info, warn, error};

use std::collections::VecDeque;

use crate::memory;

// Even though the keyboard only has (at most) 64 keys, because of the way they
// are wired up, it takes up 256 bytes of the address space.
pub const KBD_MEM_SIZE: u16 = 0x0100;

pub struct KeyboardMemory {
    key_matrix: [u8; 8],
}

impl memory::MemIO for KeyboardMemory {
    fn read_byte(&mut self, addr: u16) -> u8 {
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
            panic!("Failed read: Address offset 0x{:04X} is invalid for the keyboard", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if addr < KBD_MEM_SIZE {
            warn!("Attempted to write 0x{:02X} to address 0x{:04X} of the keyboard, this is a no-op.", val, addr);
        } else {
            panic!("Failed write: Address offset 0x{:04X} is invalid for the keyboard", addr);
        }
    }
}
impl KeyboardMemory {
    pub fn new(start_addr: u16) -> KeyboardMemory {
        let memory = KeyboardMemory {
            key_matrix: [0; 8],
        };

        info!("Created the keyboard memory interface, starting address: 0x{:04X}, spanning {} bytes.", start_addr, KBD_MEM_SIZE);
        memory
    }
}


// The representation of the keyboard actions that get applied to the data bus.
pub enum KeyboardQueueEntryAction {
    Press,
    Release,
}
pub struct KeyboardQueueEntry {
    pub action: KeyboardQueueEntryAction,
    pub row:    u8,
    pub column: u8,
    pub delay:  u32,  // Minimum delay in CPU cycles since the previous queue entry was processed.
}

pub struct KeyboardQueue {
    deque:     VecDeque<KeyboardQueueEntry>,
    cpu_delta: u32,
}

impl KeyboardQueue {
    pub fn new() -> KeyboardQueue {
        KeyboardQueue {
            deque:     VecDeque::with_capacity(4096),
            cpu_delta: 0,
        }
    }

    pub fn power_off(&mut self, _mem: &mut KeyboardMemory) {
        self.deque.clear();
        self.deque.reserve(4096);
        self.deque.shrink_to(4096);
        self.cpu_delta = 0;
    }

    pub fn add_keyboard_event(&mut self, entry: KeyboardQueueEntry) {

        self.deque.push_back(entry);
    }

    pub fn tick(&mut self, kbd_mem: &mut KeyboardMemory, cycles: u32) {

        self.cpu_delta += cycles;
        let mut entry_used = false;

        match self.deque.get(0) {
            Some(entry) => {
                if self.cpu_delta >= entry.delay {
                    match entry.action {
                        KeyboardQueueEntryAction::Press => {
                            kbd_mem.key_matrix[entry.row as usize] |= entry.column;
                        }, 
                        KeyboardQueueEntryAction::Release => {
                            kbd_mem.key_matrix[entry.row as usize] &= !entry.column;
                        }, 
                    }
                    self.cpu_delta = 0;
                    entry_used = true;
                }
            },
            None => { },
        }

        if entry_used {
            self.deque.pop_front();
        }
    }
}
