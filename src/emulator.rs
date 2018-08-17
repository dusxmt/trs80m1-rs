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

use sdl2;

use z80::cpu;
use proj_config;
use memory;
use memory::MemoryChipOps;
use keyboard;
use video;
use fonts;
use timing;
use user_interface;
use util;
use util::MessageLogging;


pub struct Emulator {
    pub cpu:                  cpu::CPU,
    input_system:             keyboard::InputSystem,
    video_system:             video::VideoSystem,

    pub fullscreen:           bool,
    pub powered_on:           bool,
    pub paused:               bool,
    pub exit_request:         bool,
    pub video_config_update:  bool,
    pub resolution_update:    bool,
    pub scheduler_update:     bool,
}

impl Emulator {
    pub fn new(config_items: &proj_config::ConfigItems, _startup_logger: &mut util::StartupLogger) -> Emulator {

        let (red, green, blue) = config_items.video_bg_color;
        let bg_color = Emulator::rgb888_into_rgb332(red, green, blue);

        let (red, green, blue) = config_items.video_fg_color;
        let fg_color = Emulator::rgb888_into_rgb332(red, green, blue);

        let font = Emulator::font_for_character_generator(config_items.video_character_generator);

        Emulator {
            cpu:                 cpu::CPU::new(),
            input_system:        keyboard::InputSystem::new(),
            video_system:        video::VideoSystem::new(bg_color, fg_color, font),
            fullscreen:          false,
            powered_on:          false,
            paused:              false,
            exit_request:        false,
            video_config_update: false,
            resolution_update:   false,
            scheduler_update:    false,
        }
    }
    fn rgb888_into_rgb332(red: u8, green: u8, blue: u8) -> u8 {
        (red    & 0b111_000_00) |
        ((green & 0b111_000_00) >> 3) |
        ((blue  & 0b110_000_00) >> 6)
    }
    fn font_for_character_generator(character_generator: u32) -> fonts::FontSelector {
        match character_generator {
            1 => { fonts::FontSelector::CG0 },
            2 => { fonts::FontSelector::CG1 },
            3 => { fonts::FontSelector::CG2 },
            _ => { panic!("Invalid character generator selected"); },
        }
    }
    // Runs the emulator; returns whether to ask for the user to press Enter
    // to close the program on Windows.
    pub fn run(&mut self, memory_system: &mut memory::MemorySystem, config_system: &mut proj_config::ConfigSystem, mut startup_logger: util::StartupLogger) -> bool {
        let mut in_desktop_fsm:  bool;

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
                    ns_per_frame = timing::NS_PER_FRAME;
                },
            }
        } else {
            startup_logger.log_message("Using the software rendering mode.".to_owned());
            renderer = window.renderer().software().build().expect(".expect() call: Failed to create a non-accelerated SDL2 renderer");
            ns_per_frame = timing::NS_PER_FRAME;
        }
        renderer.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT).expect(".expect() call: Failed to set the SDL2 renderer's logical size");
        in_desktop_fsm = config_system.config_items.video_desktop_fullscreen_mode;

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
        let (mut narrow_glyphs, mut wide_glyphs) = self.video_system.generate_glyph_textures(&mut renderer);

        self.full_reset(memory_system);
        let mut timer = timing::FrameTimer::new(ns_per_frame, timing::NS_PER_CPU_CYCLE);
        let mut scheduler = timing::Scheduler::new(config_system);
        loop {
            let frame_cycles = timer.frame_cycles();

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

            if self.video_config_update {
                let (red, green, blue) = config_system.config_items.video_bg_color;
                let bg_color = Emulator::rgb888_into_rgb332(red, green, blue);

                let (red, green, blue) = config_system.config_items.video_fg_color;
                let fg_color = Emulator::rgb888_into_rgb332(red, green, blue);

                let font = Emulator::font_for_character_generator(config_system.config_items.video_character_generator);
                self.video_system.update_colors_and_font(bg_color, fg_color, font);

                let (new_narrow_glyphs, new_wide_glyphs) = self.video_system.generate_glyph_textures(&mut renderer);
                narrow_glyphs = new_narrow_glyphs;
                wide_glyphs = new_wide_glyphs;

                self.video_config_update = false;
            }
            if self.resolution_update {
                if self.fullscreen && !in_desktop_fsm {
                    let window = renderer.window_mut().expect(".expect() call: Failed to get a mutable reference to the SDL2 window for the purpose of changing the fullscreen resolution");
                    let (width, height) = config_system.config_items.video_fullscreen_resolution;
                    window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size");
                } else if !self.fullscreen {
                    let window = renderer.window_mut().expect(".expect() call: Failed to get a mutable reference to the SDL2 window for the purpose of changing the windowed resolution");
                    let (width, height) = config_system.config_items.video_windowed_resolution;
                    window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size");
                }
                self.resolution_update = false;
            }
            if self.scheduler_update {
                scheduler.update(config_system);
                self.scheduler_update = false;
            }
            if memory_system.cas_rec.config_update_request {
                memory_system.cas_rec.handle_config_update_request(config_system);
            }
            if self.powered_on && !self.paused {
                scheduler.perform_cycles(frame_cycles,
                                         &mut self.cpu,
                                         &mut self.input_system,
                                         memory_system);
            }
            user_interface.collect_logged_messages(self, memory_system);
            user_interface.update_screen(&self);

            // Handle fullscreen requests:
            if self.input_system.fullscreen_request ^ self.fullscreen {
                let window = renderer.window_mut().expect(".expect() call: Failed to get a mutable reference to the SDL2 window for the purpose of changing the fullscreen status");
                match self.input_system.fullscreen_request {
                    true => {
                        in_desktop_fsm = config_system.config_items.video_desktop_fullscreen_mode;

                        if !in_desktop_fsm {
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

            timer.frame_next();
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
