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

use sdl2;

use crate::fonts;
use crate::video::*;


fn rgb888_into_rgb332(red: u8, green: u8, blue: u8) -> u8 {
    (red    & 0b111_000_00) |
    ((green & 0b111_000_00) >> 3) |
    ((blue  & 0b110_000_00) >> 6)
}

fn font_for_cg_num(character_generator: u32) -> &'static [u8] {
    match character_generator {
        1 => { &fonts::FONT_CG0 },
        2 => { &fonts::FONT_CG1 },
        3 => { &fonts::FONT_CG2 },
        _ => { panic!("Invalid character generator selected"); },
    }
}

// Generate textures for the screen tiles.
pub fn generate_glyph_textures<'t>(video_bg_color:  (u8, u8, u8),
                                   video_fg_color:  (u8, u8, u8),
                                   video_character_generator: u32,
                                   texture_creator: &'t sdl2::render::TextureCreator<sdl2::video::WindowContext>)
           -> (Box<[sdl2::render::Texture<'t>]>, Box<[sdl2::render::Texture<'t>]>) {

    let mut narrow: Vec<sdl2::render::Texture> = Vec::new();
    let mut wide:   Vec<sdl2::render::Texture> = Vec::new();

    let (red, green, blue) = video_bg_color;
    let bg_color = rgb888_into_rgb332(red, green, blue);

    let (red, green, blue) = video_fg_color;
    let fg_color = rgb888_into_rgb332(red, green, blue);

    let font = font_for_cg_num(video_character_generator);


    for glyph_iter in 0..256 {
        let mut texture = texture_creator.create_texture(sdl2::pixels::PixelFormatEnum::RGB332,
            sdl2::render::TextureAccess::Static, GLYPH_WIDTH, GLYPH_HEIGHT_S).unwrap();
        let font_glyph: &[u8];
        if (glyph_iter & 0x80) == 0 {
            let font_index = ((glyph_iter as u32) * fonts::FONT_GLYPH_BYTES) as usize;
            font_glyph = &font[font_index..(font_index + (fonts::FONT_GLYPH_BYTES as usize))];
        } else {
            let graph_index = (((glyph_iter & 0b0011_1111) as u32) * fonts::FONT_GLYPH_BYTES) as usize;
            font_glyph = &fonts::GRAPH_FONT[graph_index..(graph_index + (fonts::FONT_GLYPH_BYTES as usize))];
        }
        assert!(font_glyph.len() == (GLYPH_HEIGHT as usize));

        let mut pixel_data: [u8; (GLYPH_WIDTH * GLYPH_HEIGHT_S) as usize] = [bg_color; (GLYPH_WIDTH * GLYPH_HEIGHT_S) as usize];

        for glyph_y in 0..(GLYPH_HEIGHT as usize) {
            let glyph_scanline = font_glyph[glyph_y];
            for glyph_x in 0..(GLYPH_WIDTH as usize) {
                let x_offset = glyph_x;
                let y_offset = glyph_y * 2;

                if (glyph_scanline & (1 << (glyph_x))) != 0 {
                    pixel_data[(y_offset * (GLYPH_WIDTH as usize)) + x_offset] = fg_color;
                    pixel_data[((y_offset + 1) * (GLYPH_WIDTH as usize)) + x_offset] = fg_color;
                }
            }
        }
        texture.update(None, &pixel_data, GLYPH_WIDTH as usize).unwrap();

        narrow.push(texture);
    }
    for glyph_iter in 0..256 {
        let mut texture = texture_creator.create_texture(sdl2::pixels::PixelFormatEnum::RGB332,
            sdl2::render::TextureAccess::Static, GLYPH_WIDTH_W, GLYPH_HEIGHT_S).unwrap();
        let font_glyph: &[u8];
        if (glyph_iter & 0x80) == 0 {
            let font_index = ((glyph_iter as u32) * fonts::FONT_GLYPH_BYTES) as usize;
            font_glyph = &font[font_index..(font_index + (fonts::FONT_GLYPH_BYTES as usize))];
        } else {
            let graph_index = (((glyph_iter & 0b0011_1111) as u32) * fonts::FONT_GLYPH_BYTES) as usize;
            font_glyph = &fonts::GRAPH_FONT[graph_index..(graph_index + (fonts::FONT_GLYPH_BYTES as usize))];
        }
        assert!(font_glyph.len() == (GLYPH_HEIGHT as usize));

        let mut pixel_data: [u8; (GLYPH_WIDTH_W * GLYPH_HEIGHT_S) as usize] = [bg_color; (GLYPH_WIDTH_W * GLYPH_HEIGHT_S) as usize];

        for glyph_y in 0..(GLYPH_HEIGHT as usize) {
            let glyph_scanline = font_glyph[glyph_y];
            for glyph_x in 0..(GLYPH_WIDTH as usize) {
                let x_offset = glyph_x * 2;
                let y_offset = glyph_y * 2;

                if (glyph_scanline & (1 << (glyph_x))) != 0 {
                    pixel_data[(y_offset * (GLYPH_WIDTH_W as usize)) + x_offset] = fg_color;
                    pixel_data[(y_offset * (GLYPH_WIDTH_W as usize)) + x_offset + 1] = fg_color;
                    pixel_data[((y_offset + 1) * (GLYPH_WIDTH_W as usize)) + x_offset] = fg_color;
                    pixel_data[((y_offset + 1) * (GLYPH_WIDTH_W as usize)) + x_offset + 1] = fg_color;
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

// Render the screen contents:
pub fn render(canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
              narrow: &Box<[sdl2::render::Texture]>,
              wide: &Box<[sdl2::render::Texture]>,
              frame: &VideoFrame) {

    canvas.clear();
    if !frame.modesel {
        for glyph_y in 0..SCREEN_ROWS {
            for glyph_x in 0..SCREEN_COLS {
                let glyph_texture = &narrow[frame.memory[((glyph_y * SCREEN_COLS) as usize) + (glyph_x as usize)] as usize];
                let dest = sdl2::rect::Rect::new((glyph_x as i32) * (GLYPH_WIDTH as i32), (glyph_y as i32) * (GLYPH_HEIGHT_S as i32), GLYPH_WIDTH, GLYPH_HEIGHT_S);
                canvas.copy(glyph_texture, None, Some(dest)).unwrap();
            }
        }
    } else {
        for glyph_y in 0..SCREEN_ROWS {
            for glyph_x in 0..SCREEN_COLS_W {
                let glyph_texture = &wide[frame.memory[((glyph_y * SCREEN_COLS) as usize) + ((glyph_x * 2) as usize)] as usize];
                let dest = sdl2::rect::Rect::new((glyph_x as i32) * (GLYPH_WIDTH_W as i32), (glyph_y as i32) * (GLYPH_HEIGHT_S as i32), GLYPH_WIDTH_W, GLYPH_HEIGHT_S);
                canvas.copy(glyph_texture, None, Some(dest)).unwrap();
            }
        }
    }
    canvas.present();
}
