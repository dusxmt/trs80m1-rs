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

use crate::memory;
use crate::util::Sink;

pub const VID_MEM_SIZE:    u16 = 0x0400;

pub const SCREEN_ROWS:     u32 = 16;
pub const SCREEN_COLS:     u32 = 64; // When `modesel == false`.
pub const SCREEN_COLS_W:   u32 = 32; // When `modesel == true`.
pub const GLYPH_HEIGHT:    u32 = 12;               // Glyphs are displayed twice
pub const GLYPH_HEIGHT_S:  u32 = GLYPH_HEIGHT * 2; // as tall on the screen.
pub const GLYPH_WIDTH:     u32 = 8;  // When `modesel == false`.
pub const GLYPH_WIDTH_W:   u32 = 16; // When `modesel == true`.
pub const SCREEN_HEIGHT:   u32 = SCREEN_ROWS * GLYPH_HEIGHT_S;
pub const SCREEN_WIDTH:    u32 = SCREEN_COLS * GLYPH_WIDTH;

pub struct VideoMemory {
    memory:        [u8; VID_MEM_SIZE as usize],
    pub modesel:   bool, // true => 32-columns; false => 64-columns.
    lowercase_mod: bool,
}

pub struct VideoFrame {
    pub memory:   [u8; VID_MEM_SIZE as usize],
    pub modesel:  bool, // true => 32-columns; false => 64-columns.
}

impl VideoFrame {
    pub fn new(memory: &VideoMemory) -> VideoFrame {
        VideoFrame {
            memory:  memory.memory.clone(),
            modesel: memory.modesel,
        }
    }
}

impl memory::MemIO for VideoMemory {
    fn read_byte(&mut self, addr: u16) -> u8 {
        if addr < VID_MEM_SIZE {
            self.memory[addr as usize]
        } else {
            panic!("Failed read: Address offset 0x{:04X} is invalid for the video RAM", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if addr < VID_MEM_SIZE {
            let mut to_set = val;

            // If the lowercase mod is not installed, simulate the missing
            // bit 6 of the video RAM as b6 = !b5 & !b7
            if !self.lowercase_mod {
                if (val & 0b1010_0000) != 0 {
                    to_set &= !(0b0100_0000);
                } else {
                    to_set |=   0b0100_0000;
                }
            }
            if self.memory[addr as usize] != to_set {
                self.memory[addr as usize] = to_set;
            }
        } else {
            panic!("Failed write: Address offset 0x{:04X} is invalid for the video RAM", addr);
        }
    }
}

impl VideoMemory {
    pub fn new(lowercase_mod: bool, start_addr: u16) -> VideoMemory {
        let video_memory = VideoMemory {
            memory:        [0; VID_MEM_SIZE as usize],
            modesel:       false,
            lowercase_mod,
        };
        info!("Created the video memory, starting address: 0x{:04X}, spanning {} bytes.", start_addr, VID_MEM_SIZE);
        video_memory
    }
    pub fn power_off(&mut self) {

        let size = self.memory.len();
        let mut index = 0;

        while index < size {
            self.memory[index] = 0;
            index += 1;
        }

        info!("The video ram was cleared.");
    }
    pub fn update_lowercase_mod(&mut self, new_value: bool) {
        self.lowercase_mod = new_value;
    }
}

pub struct Video {
    cpu_delta:        u32,
    cycles_per_frame: u32,
}

impl Video {
    pub fn new(cycles_per_frame: u32) -> Video {
        Video {
            cpu_delta:  0,
            cycles_per_frame,
        }
    }
    pub fn power_off(&mut self, mem: &mut VideoMemory) {
        self.cpu_delta = 0;
        mem.power_off();
    }
    pub fn tick<VS: Sink<VideoFrame>>(&mut self, vid_mem: &VideoMemory, cpu_cycles: u32, video_frame_sink: &mut VS) {
        self.cpu_delta += cpu_cycles;
        if self.cpu_delta >= self.cycles_per_frame {
            self.cpu_delta -= self.cycles_per_frame;
            video_frame_sink.push(VideoFrame::new(vid_mem));
        }
    }
}
