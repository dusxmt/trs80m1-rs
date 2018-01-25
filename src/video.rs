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

use fonts;
use memory;
use sdl2;
use util::MessageLogging;

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
    needs_update:  bool,
    pub modesel:   bool, // true => 32-columns; false => 64-columns.
    lowercase_mod: bool,

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl memory::MemIO for VideoMemory {
    fn read_byte(&mut self, addr: u16, _cycle_timestamp: u32) -> u8 {
        if addr < VID_MEM_SIZE {
            self.memory[addr as usize]
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for the video RAM", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8, _cycle_timestamp: u32) {
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
                self.needs_update = true;
            }
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for the video RAM", addr);
        }
    }
}

impl VideoMemory {
    pub fn new(lowercase_mod: bool, start_addr: u16) -> VideoMemory {
        let mut video_memory = VideoMemory {
                                   memory:            [0; VID_MEM_SIZE as usize],
                                   modesel:           false,
                                   needs_update:      true, // Since we haven't drawn anything before yet.
                                   lowercase_mod:     lowercase_mod,

                                   logged_messages:   Vec::new(),
                                   messages_present:  false,
                               };
        video_memory.log_message(format!("Created the video memory, starting address: 0x{:04X}, spanning {} bytes.", start_addr, VID_MEM_SIZE));
        video_memory
    }
    pub fn power_off(&mut self) {

        let size = self.memory.len();
        let mut index = 0;

        while index < size {
            self.memory[index] = 0;
            index += 1;
        }

        self.log_message("The video ram was cleared.".to_owned());
    }
}

pub struct VideoSystem {
    bg_color:          u8,  // Of the `RGB332` format.
    fg_color:          u8,  // Of the `RGB332` format.
    font:              [u8; fonts::FONT_SIZE],

    logged_messages:   Vec<String>,
    messages_present:  bool,
}

impl VideoSystem {
    pub fn new(bg_color: u8, fg_color: u8, font: fonts::FontSelector)
        -> VideoSystem {
        VideoSystem {
            bg_color:          bg_color,
            fg_color:          fg_color,
            font:              match font {
                                   fonts::FontSelector::CG0 => fonts::FONT_CG0,
                                   fonts::FontSelector::CG1 => fonts::FONT_CG1,
                                   fonts::FontSelector::CG2 => fonts::FONT_CG2,
                               },
            logged_messages:   Vec::new(),
            messages_present:  false,
        }
    }
    // Generate textures for the screen tiles.
    pub fn generate_glyph_textures(&self, renderer: &mut sdl2::render::Renderer)
        -> (Box<[sdl2::render::Texture]>, Box<[sdl2::render::Texture]>) {
        let mut narrow: Vec<sdl2::render::Texture> = Vec::new();
        let mut wide:   Vec<sdl2::render::Texture> = Vec::new();

        for glyph_iter in 0..256 {
            let mut texture = renderer.create_texture(sdl2::pixels::PixelFormatEnum::RGB332,
                sdl2::render::TextureAccess::Static, GLYPH_WIDTH, GLYPH_HEIGHT_S).unwrap();
            let font_glyph: &[u8];
            if (glyph_iter & 0x80) == 0 {
                let font_index = ((glyph_iter as u32) * fonts::FONT_GLYPH_BYTES) as usize;
                font_glyph = &self.font[font_index..(font_index + (fonts::FONT_GLYPH_BYTES as usize))];
            } else {
                let graph_index = (((glyph_iter & 0b0011_1111) as u32) * fonts::FONT_GLYPH_BYTES) as usize;
                font_glyph = &fonts::GRAPH_FONT[graph_index..(graph_index + (fonts::FONT_GLYPH_BYTES as usize))];
            }
            assert!(font_glyph.len() == (GLYPH_HEIGHT as usize));

            let mut pixel_data: [u8; (GLYPH_WIDTH * GLYPH_HEIGHT_S) as usize] = [self.bg_color; (GLYPH_WIDTH * GLYPH_HEIGHT_S) as usize];

            for glyph_y in 0..(GLYPH_HEIGHT as usize) {
                let glyph_scanline = font_glyph[glyph_y];
                for glyph_x in 0..(GLYPH_WIDTH as usize) {
                    let x_offset = glyph_x;
                    let y_offset = glyph_y * 2;

                    if (glyph_scanline & (1 << (glyph_x))) != 0 {
                        pixel_data[(y_offset * (GLYPH_WIDTH as usize)) + x_offset] = self.fg_color;
                        pixel_data[((y_offset + 1) * (GLYPH_WIDTH as usize)) + x_offset] = self.fg_color;
                    }
                }
            }
            texture.update(None, &pixel_data, GLYPH_WIDTH as usize).unwrap();

            narrow.push(texture);
        }
        for glyph_iter in 0..256 {
            let mut texture = renderer.create_texture(sdl2::pixels::PixelFormatEnum::RGB332,
                sdl2::render::TextureAccess::Static, GLYPH_WIDTH_W, GLYPH_HEIGHT_S).unwrap();
            let font_glyph: &[u8];
            if (glyph_iter & 0x80) == 0 {
                let font_index = ((glyph_iter as u32) * fonts::FONT_GLYPH_BYTES) as usize;
                font_glyph = &self.font[font_index..(font_index + (fonts::FONT_GLYPH_BYTES as usize))];
            } else {
                let graph_index = (((glyph_iter & 0b0011_1111) as u32) * fonts::FONT_GLYPH_BYTES) as usize;
                font_glyph = &fonts::GRAPH_FONT[graph_index..(graph_index + (fonts::FONT_GLYPH_BYTES as usize))];
            }
            assert!(font_glyph.len() == (GLYPH_HEIGHT as usize));

            let mut pixel_data: [u8; (GLYPH_WIDTH_W * GLYPH_HEIGHT_S) as usize] = [self.bg_color; (GLYPH_WIDTH_W * GLYPH_HEIGHT_S) as usize];

            for glyph_y in 0..(GLYPH_HEIGHT as usize) {
                let glyph_scanline = font_glyph[glyph_y];
                for glyph_x in 0..(GLYPH_WIDTH as usize) {
                    let x_offset = glyph_x * 2;
                    let y_offset = glyph_y * 2;

                    if (glyph_scanline & (1 << (glyph_x))) != 0 {
                        pixel_data[(y_offset * (GLYPH_WIDTH_W as usize)) + x_offset] = self.fg_color;
                        pixel_data[(y_offset * (GLYPH_WIDTH_W as usize)) + x_offset + 1] = self.fg_color;
                        pixel_data[((y_offset + 1) * (GLYPH_WIDTH_W as usize)) + x_offset] = self.fg_color;
                        pixel_data[((y_offset + 1) * (GLYPH_WIDTH_W as usize)) + x_offset + 1] = self.fg_color;
                    }
                }
            }
            texture.update(None, &pixel_data, GLYPH_WIDTH_W as usize).unwrap();

            wide.push(texture);
        }

        assert!(narrow.len() == 256);
        assert!(wide.len() == 256);
        (narrow.into_boxed_slice(), wide.into_boxed_slice())
    }
    pub fn render(&mut self, renderer: &mut sdl2::render::Renderer,
                  narrow: &Box<[sdl2::render::Texture]>,
                  wide: &Box<[sdl2::render::Texture]>,
                  memory_system: &mut memory::MemorySystem) {
        let ref mut vid_mem = memory_system.vid_mem;

        renderer.clear();
        if !vid_mem.modesel {
            for glyph_y in 0..SCREEN_ROWS {
                for glyph_x in 0..SCREEN_COLS {
                    let glyph_texture = &narrow[vid_mem.memory[((glyph_y * SCREEN_COLS) as usize) + (glyph_x as usize)] as usize];
                    let dest = sdl2::rect::Rect::new((glyph_x as i32) * (GLYPH_WIDTH as i32), (glyph_y as i32) * (GLYPH_HEIGHT_S as i32), GLYPH_WIDTH, GLYPH_HEIGHT_S);
                    renderer.copy(glyph_texture, None, Some(dest)).unwrap();
                }
            }
        } else {
            for glyph_y in 0..SCREEN_ROWS {
                for glyph_x in 0..SCREEN_COLS_W {
                    let glyph_texture = &wide[vid_mem.memory[((glyph_y * SCREEN_COLS) as usize) + ((glyph_x * 2) as usize)] as usize];
                    let dest = sdl2::rect::Rect::new((glyph_x as i32) * (GLYPH_WIDTH_W as i32), (glyph_y as i32) * (GLYPH_HEIGHT_S as i32), GLYPH_WIDTH_W, GLYPH_HEIGHT_S);
                    renderer.copy(glyph_texture, None, Some(dest)).unwrap();
                }
            }
        }
        renderer.present();
    }
}

impl MessageLogging for VideoMemory {
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

impl MessageLogging for VideoSystem {
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
