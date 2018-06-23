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

use std::thread;
use std::time as std_time;

use sdl2;

use z80::cpu;
use proj_config;
use memory;
use memory::MemoryChipOps;
use keyboard;
use video;
use fonts;
use time;
use user_interface;
use util;
use util::MessageLogging;

// Timing description:
const     MASTER_HZ:            u32 = 10_644_480;
const     FRAME_RATE:           u32 = MASTER_HZ     / 177_408;
const     NS_PER_FRAME:         u32 = 1_000_000_000 / FRAME_RATE;
pub const CPU_HZ:               u32 = MASTER_HZ     / 6;
const     NS_PER_CPU_CYCLE:     u32 = 1_000_000_000 / CPU_HZ;

pub struct Emulator {
    pub cpu:             cpu::CPU,
    input_system:        keyboard::InputSystem,
    video_system:        video::VideoSystem,
    fullscreen:          bool,
    desktop_fullscreen:  bool,

    pub powered_on:      bool,
    pub paused:          bool,
    pub exit_request:    bool,

    // Keyboard timing information:
    cycles_per_keypress: u32,
    cycles_since_last:   u32,
}

impl Emulator {
    pub fn new(config_items: &proj_config::ConfigItems, _startup_logger: &mut util::StartupLogger) -> Emulator {

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
            _ => { panic!("Invalid character generator selected"); },
        };

        Emulator {
            cpu:                 cpu::CPU::new(),
            input_system:        keyboard::InputSystem::new(),
            video_system:        video::VideoSystem::new(bg_color, fg_color, font),
            fullscreen:          false,
            desktop_fullscreen:  config_items.video_desktop_fullscreen_mode,
            powered_on:          false,
            paused:              false,
            exit_request:        false,
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
    // Runs the emulator; returns whether to ask for the user to press Enter
    // to close the program on Windows.
    pub fn run(&mut self, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem, mut startup_logger: util::StartupLogger) -> bool {
        let mut frame_begin:     std_time::Duration;
        let mut frame_end:       std_time::Duration;
        let mut last_frame_ns:   u32;
        let mut residual_ns:     u32;
        let mut frame_cycles:    u32;

        // Initialize SDL:
        let sdl = sdl2::init().expect(".expect() call: Failed to initialize SDL2");
        let video = sdl.video().expect(".expect() call: Failed to initialize the SDL2 video subsystem");
        let mut event_pump = sdl.event_pump().expect(".expect() call: Failed to initialize the SDL2 event pump");
        sdl.mouse().show_cursor(false);

        // Create a rendering context:
        let (width, height) = config_system.config_items.video_windowed_resolution;
        let mut window_builder = video.window("trs80m1-rs", width, height);
        let window = window_builder.position_centered().build().expect(".expect() call: Failed to create the SDL2 window");

        let mut renderer: sdl2::render::Renderer;
        let ns_per_frame: u32;
        if config_system.config_items.video_use_hw_accel {
            match window.display_mode() {
                Ok(mode) => {
                    renderer = window.renderer().accelerated().present_vsync().build().expect(".expect() call: Failed to create an accelerated SDL2 renderer with vsync");
                    let fallback_refresh_rate = (mode.refresh_rate as u32) + 10;
                    ns_per_frame = 1_000_000_000 / fallback_refresh_rate;
                    startup_logger.log_message(format!("SDL reports a display refresh rate of {}Hz; using vsync, setting software fallback framerate throttle to {} FPS.", mode.refresh_rate, fallback_refresh_rate));
                },
                Err(err) => {
                    startup_logger.log_message(format!("Failed to get the display mode: {}.", err));
                    startup_logger.log_message("Assuming that vsync doesn't work.".to_owned());
                    renderer = window.renderer().accelerated().build().expect(".expect() call: Failed to create an accelerated SDL2 renderer without vsync");
                    ns_per_frame = NS_PER_FRAME;
                },
            }
        } else {
            startup_logger.log_message("Using the software rendering mode.".to_owned());
            renderer = window.renderer().software().build().expect(".expect() call: Failed to create a non-accelerated SDL2 renderer");
            ns_per_frame = NS_PER_FRAME;
        }
        renderer.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT).expect(".expect() call: Failed to set the SDL2 renderer's logical size");

        startup_logger.log_message("Switching to the curses-based user interface.".to_owned());
        let mut user_interface = match user_interface::UserInterface::new() {
            Some(user_interface) => {
                user_interface
            },
            None => {
                eprintln!("Starting the curses-based user interface failed.");
                return true;
            },
        };
        user_interface.consume_startup_logger(startup_logger);

        // Generate textures for the screen glyphs:
        let (narrow_glyphs, wide_glyphs) = self.video_system.generate_glyph_textures(&mut renderer);

        self.full_reset(memory_system);
        let stamp_now = time::get_time();
        frame_begin = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);

        last_frame_ns = ns_per_frame;
        loop {
            // Execute as many machine cycles as we should've executed on the
            // last frame.
            frame_cycles = last_frame_ns / NS_PER_CPU_CYCLE;
            residual_ns  = last_frame_ns % NS_PER_CPU_CYCLE;
            self.input_system.handle_events(&mut self.exit_request, &mut event_pump);
            if self.exit_request { return false; }
            if self.input_system.reset_request {
                user_interface.execute_command("machine reset full", self, memory_system, config_system);
                self.input_system.reset_request = false;
            }
            if self.input_system.pause_request {
                user_interface.execute_command("machine pause toggle", self, memory_system, config_system);
                self.input_system.pause_request = false;
            }

            user_interface.handle_user_input(self, memory_system, config_system);
            if self.exit_request { return true; }

            if self.powered_on && !self.paused {
                self.emulate_cycles(frame_cycles, memory_system);
            }
            user_interface.collect_logged_messages(self, memory_system);
            user_interface.update_screen(&self);

            // Handle fullscreen requests:
            if self.input_system.fullscreen_request ^ self.fullscreen {
                let window = renderer.window_mut().expect(".expect() call: Failed to get a mutable reference to the SDL2 window for the purpose of changing the fullscreen status");
                match self.input_system.fullscreen_request {
                    true => {
                        if !self.desktop_fullscreen {
                            let (width, height) = config_system.config_items.video_fullscreen_resolution;
                            window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size when going to fullscreen");
                            window.set_fullscreen(sdl2::video::FullscreenType::True).expect(".expect() call: Failed to set the SDL2 window to true fullscreen");
                        } else {
                            window.set_fullscreen(sdl2::video::FullscreenType::Desktop).expect(".expect() call: Failed to set the SDL2 window to desktop fullscreen");
                        }
                    },
                    false => {
                        window.set_fullscreen(sdl2::video::FullscreenType::Off).expect(".expect() call: Failed to set the SDL2 window to windowed mode.");
                        let (width, height) = config_system.config_items.video_windowed_resolution;
                        window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size when going to windowed mode");
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

            // Take care of the remaining time from the frame before this one
            // that was too short to execute any cycles:
            last_frame_ns += residual_ns;

            // Our end is someone else's beginning.
            frame_begin = frame_end;
        }
    }
    pub fn power_on(&mut self, _memory_system: &mut memory::MemorySystem) {
        self.cpu.full_reset();
        self.powered_on = true;
    }
    pub fn power_off(&mut self, memory_system: &mut memory::MemorySystem) {
        self.cpu.power_off();

        memory_system.ram_chip.wipe();
        memory_system.vid_mem.power_off();
        memory_system.cas_rec.power_off();

        memory_system.nmi_request = false;
        memory_system.int_request = false;
        
        self.powered_on = false;
    }
    pub fn reset(&mut self, _memory_system: &mut memory::MemorySystem) {
        self.cpu.reset();
    }
    pub fn full_reset(&mut self, memory_system: &mut memory::MemorySystem) {
        self.power_off(memory_system);
        self.power_on(memory_system);
    }
}

impl MessageLogging for Emulator {
    fn log_message(&mut self, _message: String) {
        unreachable!();
    }
    fn messages_available(&self) -> bool {
        self.cpu.messages_available()
            || self.input_system.messages_available()
            || self.video_system.messages_available()
    }
    fn collect_messages(&mut self) -> Vec<String> {
        let mut logged_thus_far: Vec<String> = self.cpu.collect_messages();
        logged_thus_far.append(&mut self.input_system.collect_messages());
        logged_thus_far.append(&mut self.video_system.collect_messages());

        logged_thus_far
    }
}
