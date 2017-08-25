// Copyright (c) 2017 Marek Benc <dusxmt@gmx.com>
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

use std::thread;
use std::time as std_time;

use sdl2;

use z80::cpu;
use proj_config;
use memory;
use keyboard;
use video;
use fonts;
use time;

// Timing description:
const     MASTER_HZ:            u32 = 10_644_480;
const     FRAME_RATE:           u32 = MASTER_HZ     / 177_408;
const     NS_PER_FRAME:         u32 = 1_000_000_000 / FRAME_RATE;
pub const CPU_HZ:               u32 = MASTER_HZ     / 6;
const     NS_PER_CPU_CYCLE:     u32 = 1_000_000_000 / CPU_HZ;

pub struct Emulator {
    cpu:                 cpu::CPU,
    input_system:        keyboard::InputSystem,
    video_system:        video::VideoSystem,
    running:             bool,
    fullscreen:          bool,
    desktop_fullscreen:  bool,

    // Keyboard timing information:
    cycles_per_keypress: u32,
    cycles_since_last:   u32,
}

impl Emulator {
    pub fn new(config_items: &proj_config::ConfigItems) -> Emulator {

        // Convert the RGB color values into a single RGB332 value:
        let (red, green, blue) = config_items.video_bg_color;
        let bg_color = (red    & 0b111_000_00) |
                       ((green & 0b111_000_00) >> 3) |
                       ((blue  & 0b110_000_00) >> 6);

        let (red, green, blue) = config_items.video_fg_color;
        let fg_color = (red    & 0b111_000_00) |
                       ((green & 0b111_000_00) >> 3) |
                       ((blue  & 0b110_000_00) >> 6);

        // Determine the selected font:
        let font = match config_items.video_character_generator {
            1 => { fonts::FontSelector::CG0 },
            2 => { fonts::FontSelector::CG1 },
            3 => { fonts::FontSelector::CG2 },
            _ => { panic!("Invalid character generator selected."); },
        };

        Emulator {
            cpu:                 cpu::CPU::new(),
            input_system:        keyboard::InputSystem::new(),
            video_system:        video::VideoSystem::new(bg_color, fg_color, font),
            running:             true,
            fullscreen:          false,
            desktop_fullscreen:  config_items.video_desktop_fullscreen_mode,
            cycles_per_keypress: (CPU_HZ * config_items.keyboard_ms_per_keypress) / 1000,
            cycles_since_last:   0,
        }
    }
    pub fn emulate_cycles(&mut self, cycles_to_exec: u32, memory_system: &mut memory::MemorySystem) {
        let mut needed_cycles = cycles_to_exec;

        while (self.cycles_since_last + needed_cycles) > self.cycles_per_keypress {
            let to_exec = self.cycles_per_keypress - self.cycles_since_last;

            self.cpu.exec(to_exec, memory_system);
            self.input_system.update_keyboard(memory_system);

            self.cycles_since_last = 0;
            needed_cycles -= to_exec;
        }
        if needed_cycles > 0 {
            self.cpu.exec(needed_cycles, memory_system);
            self.cycles_since_last += needed_cycles;
        }
    }
    pub fn run(&mut self, memory_system: &mut memory::MemorySystem, config_items: &proj_config::ConfigItems) {
        let mut frame_begin:     std_time::Duration;
        let mut frame_end:       std_time::Duration;
        let mut last_frame_ns:   u32;
        let mut frame_cycles:    u32;

        // Initialize SDL:
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let mut event_pump = sdl.event_pump().unwrap();
        sdl.mouse().show_cursor(false);

        // Create a rendering context:
        let (width, height) = config_items.video_windowed_resolution;
        let mut window_builder = video.window("trs80m1-rs", width, height);
        let window = window_builder.position_centered().build().unwrap();

        let mut renderer: sdl2::render::Renderer;
        let ns_per_frame: u32;
        if config_items.video_use_hw_accel {
            match window.display_mode() {
                Ok(mode) => {
                    renderer = window.renderer().accelerated().present_vsync().build().unwrap();
                    let fallback_refresh_rate = (mode.refresh_rate as u32) + 10;
                    ns_per_frame = 1_000_000_000 / fallback_refresh_rate;
                    println!("SDL reports a display refresh rate of {}Hz; using vsync, setting software fallback framerate throttle to {} FPS.", mode.refresh_rate, fallback_refresh_rate);
                },
                Err(err) => {
                    println!("Failed to get the display mode: {}.", err);
                    println!("Assuming that vsync doesn't work.");
                    renderer = window.renderer().accelerated().build().unwrap();
                    ns_per_frame = NS_PER_FRAME;
                },
            }
        } else {
            println!("Using the software rendering mode.");
            renderer = window.renderer().software().build().unwrap();
            ns_per_frame = NS_PER_FRAME;
        }
        renderer.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT).unwrap();

        // Generate textures for the screen glyphs:
        let (narrow_glyphs, wide_glyphs) = self.video_system.generate_glyph_textures(&mut renderer);

        let stamp_now = time::get_time();
        frame_begin = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);
        //frame_vorsleep = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);

        last_frame_ns = ns_per_frame;
        while self.running {
            // Execute as many machine cycles as we should've executed on the
            // last frame.
            frame_cycles = last_frame_ns / NS_PER_CPU_CYCLE;
            self.input_system.handle_events(&mut self.running, &mut event_pump);
            self.emulate_cycles(frame_cycles, memory_system);

            // Handle fullscreen requests:
            if self.input_system.fullscreen_request ^ self.fullscreen {
                let window = renderer.window_mut().unwrap();
                match self.input_system.fullscreen_request {
                    true => {
                        if !self.desktop_fullscreen {
                            let (width, height) = config_items.video_fullscreen_resolution;
                            window.set_size(width, height).unwrap();
                            window.set_fullscreen(sdl2::video::FullscreenType::True).unwrap();
                        } else {
                            window.set_fullscreen(sdl2::video::FullscreenType::Desktop).unwrap();
                        }
                    },
                    false => {
                        window.set_fullscreen(sdl2::video::FullscreenType::Off).unwrap();
                        let (width, height) = config_items.video_windowed_resolution;
                        window.set_size(width, height).unwrap();
                        window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);
                    }
                }
                self.fullscreen = self.input_system.fullscreen_request;
            }

            self.video_system.render(&mut renderer, &narrow_glyphs, &wide_glyphs, memory_system);

            let stamp_now = time::get_time();
            frame_end = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);
            let mut frame_duration = frame_end - frame_begin;

            // If we have time to spare, take a nap.
            let frame_dur_ns = frame_duration.subsec_nanos();
            if frame_duration.as_secs() == 0 &&
                frame_dur_ns < ns_per_frame {

                thread::sleep(std_time::Duration::new(0, ns_per_frame - frame_dur_ns));
                let stamp_now = time::get_time();
                frame_end = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);
                frame_duration = frame_end - frame_begin;
            }
            if frame_duration.as_secs() == 0 {
                last_frame_ns = frame_duration.subsec_nanos();
            } else {
                // In case the frame lasted longer than a second... pretend
                // that it didn't :3
                last_frame_ns = 1_000_000_000;
            }

            // Our end is someone else's beginning.
            frame_begin = frame_end;
        }
    }
}
