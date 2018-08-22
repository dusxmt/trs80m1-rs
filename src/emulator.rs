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
use timing;
use user_interface;
use util;
use util::MessageLogging;


pub struct Devices {
    pub cpu:        cpu::CPU,
    pub keyboard:   keyboard::Keyboard,
}

impl Devices {
    pub fn new() -> Devices {
        Devices {
            cpu:       cpu::CPU::new(),
            keyboard:  keyboard::Keyboard::new(),
        }
    }
}

impl MessageLogging for Devices {
    fn log_message(&mut self, _message: String) {
        unreachable!();
    }
    fn messages_available(&self) -> bool {
        self.cpu.messages_available()
            || self.keyboard.messages_available()
    }
    fn collect_messages(&mut self) -> Vec<String> {
        let mut logged_thus_far: Vec<String> = self.cpu.collect_messages();
        logged_thus_far.append(&mut self.keyboard.collect_messages());

        logged_thus_far
    }
}


pub struct Runtime {
    pub fullscreen:             bool,
    pub powered_on:             bool,
    pub paused:                 bool,

    pub curses_exit_request:    bool,
    pub sdl_exit_request:       bool,
    pub reset_cpu_request:      bool,
    pub reset_full_request:     bool,
    pub update_rom_request:     bool,

    pub fullscreen_desired:     bool,
    pub power_desired:          bool,
    pub pause_desired:          bool,

    pub video_system_update:    bool,
    pub video_textures_update:  bool,
    pub resolution_update:      bool,
    pub scheduler_update:       bool,


    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl Runtime {
    pub fn new() -> Runtime {
        Runtime {
            fullscreen:             false,
            powered_on:             false,
            paused:                 false,

            curses_exit_request:    false,
            sdl_exit_request:       false,
            reset_cpu_request:      false,
            reset_full_request:     false,
            update_rom_request:     false,

            fullscreen_desired:     false,
            power_desired:          false,
            pause_desired:          false,

            video_system_update:    false,
            video_textures_update:  false,
            resolution_update:      false,
            scheduler_update:       false,


            logged_messages:  Vec::new(),
            messages_present: false,
        }
    }

    fn handle_updates(&mut self,
                      config_system: &mut proj_config::ConfigSystem,
                      scheduler: &mut timing::Scheduler,
                      devices: &mut Devices,
                      memory_system: &mut memory::MemorySystem) {

        if self.reset_cpu_request {
            self.reset_cpu(devices, memory_system);
        }
        if self.reset_full_request {
            self.reset_full(devices, memory_system);
        }
        if self.update_rom_request {
            self.update_rom(config_system, devices, memory_system);
        }
        if self.power_desired != self.powered_on {
            if self.power_desired {
                self.power_on(devices, memory_system);
            } else {
                self.power_off(devices, memory_system);
            }
        }
        if self.pause_desired != self.paused {
            if self.pause_desired {
                self.pause();
            } else {
                self.unpause();
            }
        }
        if memory_system.cas_rec.config_update_request {
            memory_system.cas_rec.handle_config_update_request(config_system);
        }
        if self.scheduler_update {
            scheduler.update(config_system);
            self.scheduler_update = false;
        }
    }
    fn power_on(&mut self, devices: &mut Devices, memory_system: &mut memory::MemorySystem) {
        self.power_on_perform(devices, memory_system);
        self.log_message("Machine powered on.".to_owned());
    }
    fn power_off(&mut self, devices: &mut Devices, memory_system: &mut memory::MemorySystem) {
        self.power_off_perform(devices, memory_system);
        self.log_message("Machine powered off.".to_owned());
    }
    fn power_on_perform(&mut self, devices: &mut Devices, _memory_system: &mut memory::MemorySystem) {
        devices.cpu.full_reset();
        self.powered_on = true;
    }
    fn power_off_perform(&mut self, devices: &mut Devices, memory_system: &mut memory::MemorySystem) {
        devices.cpu.power_off();

        memory_system.ram_chip.wipe();
        memory_system.vid_mem.power_off();
        memory_system.cas_rec.power_off();

        memory_system.nmi_request = false;
        memory_system.int_request = false;

        self.powered_on = false;
    }
    fn pause(&mut self) {
        self.paused = true;
        self.log_message("Emulation paused.".to_owned());
    }
    fn unpause(&mut self) {
        self.paused = false;
        self.log_message("Emulation unpaused.".to_owned());
    }
    fn reset_cpu(&mut self, devices: &mut Devices, _memory_system: &mut memory::MemorySystem) {
        if self.powered_on {
            devices.cpu.reset();
            self.log_message("CPU reset performed.".to_owned());
        } else {
            self.log_message("Cannot reset a powered-off machine.".to_owned());
        }
        self.reset_cpu_request = false;
    }
    fn reset_full(&mut self, devices: &mut Devices, memory_system: &mut memory::MemorySystem) {
        if self.powered_on {
            self.power_off_perform(devices, memory_system);
            self.power_on_perform(devices, memory_system);

            self.log_message("Full system reset performed.".to_owned());
        } else {
            self.log_message("Cannot reset a powered-off machine.".to_owned());
        }
        self.reset_full_request = false;
    }
    fn update_rom(&mut self,
                  config_system: &proj_config::ConfigSystem,
                  devices: &mut Devices,
                  memory_system: &mut memory::MemorySystem) {

        let was_running = self.powered_on;
        if was_running {
            self.power_off_perform(devices, memory_system);
        }
        memory_system.rom_chip.wipe();
        memory_system.load_system_rom(config_system);
        if was_running {
            self.power_on_perform(devices, memory_system);
        }

        self.log_message("System rom changed.".to_owned());
        self.update_rom_request = false;
    }
    fn handle_updates_video(&mut self,
                            config_system: &proj_config::ConfigSystem,
                            in_desktop_fsm: &mut bool,
                            renderer: &mut sdl2::render::Renderer) {

        if self.fullscreen_desired != self.fullscreen {
            let window = renderer.window_mut().expect(".expect() call: Failed to get a mutable reference to the SDL2 window for the purpose of changing the fullscreen status");
            if self.fullscreen_desired {
                *in_desktop_fsm = config_system.config_items.video_desktop_fullscreen_mode;

                if !*in_desktop_fsm {
                    let (width, height) = config_system.config_items.video_fullscreen_resolution;
                    window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size when going to fullscreen");
                    window.set_fullscreen(sdl2::video::FullscreenType::True).expect(".expect() call: Failed to set the SDL2 window to true fullscreen");
                } else {
                    window.set_fullscreen(sdl2::video::FullscreenType::Desktop).expect(".expect() call: Failed to set the SDL2 window to desktop fullscreen");
                }

                self.fullscreen = true;

            } else {
                window.set_fullscreen(sdl2::video::FullscreenType::Off).expect(".expect() call: Failed to set the SDL2 window to windowed mode.");
                let (width, height) = config_system.config_items.video_windowed_resolution;
                window.set_size(width, height).expect(".expect() call: Failed to set the SDL2 window's size when going to windowed mode");
                window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);

                self.fullscreen = false;
            }
        }

        if self.resolution_update {
            if self.fullscreen && !*in_desktop_fsm {
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
    }
    // Run the emulator with a hardware accelerated rendering context.
    pub fn run_hw_accel(&mut self,
                         config_system:   &mut proj_config::ConfigSystem,
                         user_interface:  &mut user_interface::UserInterface,
                         scheduler:       &mut timing::Scheduler,
                         devices:         &mut Devices,
                         memory_system:   &mut memory::MemorySystem,
                         video_context:   &mut sdl2::VideoSubsystem,
                         event_pump:      &mut sdl2::EventPump) -> bool {

        let (width, height) = config_system.config_items.video_windowed_resolution;
        let mut window_builder = video_context.window("trs80m1-rs", width, height);
        let window = match window_builder.position_centered().build() {
            Ok(window) => { window },
            Err(error) => {
                self.log_message(format!("Failed to create a window for the video output: {}.", error));
                return false;
            },
        };
        self.fullscreen = false;

        let (renderer_result, ns_per_frame) = match window.display_mode() {
            Ok(mode) => {
                if mode.refresh_rate == 0 {

                    self.log_message("Screen refresh rate unknown, not using vsync.".to_owned());
                    (window.renderer().accelerated().build(), timing::NS_PER_FRAME)

                } else {

                    let fallback_refresh_rate = (mode.refresh_rate as u32) + 10;
                    let ns_per_frame = 1_000_000_000 / fallback_refresh_rate;
                    self.log_message(format!("SDL reports a display refresh rate of {}Hz; using vsync, setting software fallback framerate throttle to {} FPS.", mode.refresh_rate, fallback_refresh_rate));

                    (window.renderer().accelerated().present_vsync().build(), ns_per_frame)
                }
            },
            Err(err) => {
                self.log_message(format!("Failed to get the display mode: {}.", err));
                self.log_message("Screen refresh rate unknown, not using vsync.".to_owned());

                (window.renderer().accelerated().build(), timing::NS_PER_FRAME)
            },
        };

        let mut renderer = match renderer_result {
            Ok(renderer) => { renderer },
            Err(error) => {
                self.log_message(format!("Failed to create a hardware accelerated rendering context: {}.", error));
                return false;
            },
        };

        match renderer.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT) {
            Ok(..) => { () },
            Err(error) => {
                self.log_message(format!("Failed to set the SDL2 renderer's logical size: {}.", error));
                return false;
            },
        }

        let mut in_desktop_fsm = false;

        while !self.curses_exit_request
              && !self.sdl_exit_request
              && !self.video_system_update {

            let (narrow_glyphs, wide_glyphs) = video::generate_glyph_textures(config_system, &mut renderer);
            self.video_textures_update = false;

            self.run_with_video(config_system, user_interface, scheduler, devices, memory_system,
                                &mut renderer, &narrow_glyphs, &wide_glyphs,
                                event_pump, &mut in_desktop_fsm, ns_per_frame);
        }
        true
    }
    // Run the emulator with a non-hardware-accelerated rendering context.
    pub fn run_sw_render(&mut self,
                         config_system:   &mut proj_config::ConfigSystem,
                         user_interface:  &mut user_interface::UserInterface,
                         scheduler:       &mut timing::Scheduler,
                         devices:         &mut Devices,
                         memory_system:   &mut memory::MemorySystem,
                         video_context:   &mut sdl2::VideoSubsystem,
                         event_pump:      &mut sdl2::EventPump) -> bool {

        let (width, height) = config_system.config_items.video_windowed_resolution;
        let mut window_builder = video_context.window("trs80m1-rs", width, height);
        let window = match window_builder.position_centered().build() {
            Ok(window) => { window },
            Err(error) => {
                self.log_message(format!("Failed to create a window for the video output: {}.", error));
                return false;
            },
        };
        self.fullscreen = false;

        let mut renderer = match window.renderer().software().build() {
            Ok(renderer) => { renderer },
            Err(error) => {
                self.log_message(format!("Failed to create a non-hardware-accelerated rendering context: {}.", error));
                return false;
            },
        };

        match renderer.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT) {
            Ok(..) => { () },
            Err(error) => {
                self.log_message(format!("Failed to set the SDL2 renderer's logical size: {}.", error));
                return false;
            },
        }

        let mut in_desktop_fsm = false;

        while !self.curses_exit_request
              && !self.sdl_exit_request
              && !self.video_system_update {

            let (narrow_glyphs, wide_glyphs) = video::generate_glyph_textures(config_system, &mut renderer);
            self.video_textures_update = false;

            self.run_with_video(config_system, user_interface, scheduler, devices, memory_system,
                                &mut renderer, &narrow_glyphs, &wide_glyphs,
                                event_pump, &mut in_desktop_fsm, timing::NS_PER_FRAME);
        }
        true
    }
    fn run_with_video(&mut self,
                      config_system:   &mut proj_config::ConfigSystem,
                      user_interface:  &mut user_interface::UserInterface,
                      scheduler:       &mut timing::Scheduler,
                      devices:         &mut Devices,
                      memory_system:   &mut memory::MemorySystem,

                      renderer:        &mut sdl2::render::Renderer,
                      narrow_glyphs:   &Box<[sdl2::render::Texture]>,
                      wide_glyphs:     &Box<[sdl2::render::Texture]>,
                      event_pump:      &mut sdl2::EventPump,
                      in_desktop_fsm:  &mut bool,
                      ns_per_frame:    u32) {

        let mut timer = timing::FrameTimer::new(ns_per_frame, timing::NS_PER_CPU_CYCLE);

        while !self.curses_exit_request
              && !self.sdl_exit_request
              && !self.video_system_update
              && !self.video_textures_update {

            let frame_cycles = timer.frame_cycles();

            devices.keyboard.handle_events(self, event_pump);
            user_interface.handle_user_input(config_system, self, devices, memory_system);
            self.handle_updates(config_system, scheduler, devices, memory_system);
            self.handle_updates_video(config_system, in_desktop_fsm, renderer);

            if self.powered_on && !self.paused {
                scheduler.perform_cycles(frame_cycles, devices, memory_system);
            }

            user_interface.collect_logged_messages(self, devices, memory_system);
            user_interface.update_screen(&self, &devices);

            video::render(renderer, narrow_glyphs, wide_glyphs, memory_system);
            timer.frame_next();
        }
    }
    pub fn run_without_video(&mut self,
                             config_system:   &mut proj_config::ConfigSystem,
                             user_interface:  &mut user_interface::UserInterface,
                             scheduler:       &mut timing::Scheduler,
                             devices:         &mut Devices,
                             memory_system:   &mut memory::MemorySystem) {

        let mut timer = timing::FrameTimer::new(timing::NS_PER_FRAME, timing::NS_PER_CPU_CYCLE);

        while !self.curses_exit_request {
            let frame_cycles = timer.frame_cycles();

            user_interface.handle_user_input(config_system, self, devices, memory_system);
            self.handle_updates(config_system, scheduler, devices, memory_system);

            if self.powered_on && !self.paused {
                scheduler.perform_cycles(frame_cycles, devices, memory_system);
            }

            user_interface.collect_logged_messages(self, devices, memory_system);
            user_interface.update_screen(&self, &devices);

            timer.frame_next();
        }
    }
}

impl MessageLogging for Runtime {
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



pub enum ExitType {
    AskForEnterOnWindows(i32),
    JustExit(i32),
}

pub fn run(mut config_system:   proj_config::ConfigSystem,
           mut startup_logger:  util::StartupLogger,
           selected_rom:        u32) -> ExitType {

    let mut memory_system = match memory::MemorySystem::new(&config_system, &mut startup_logger, selected_rom) {
        Some(system) => { system },
        None => {
            eprintln!("Failed to initialize the emulator's memory system.");
            return ExitType::AskForEnterOnWindows(1);
        },
    };

    let mut scheduler = timing::Scheduler::new(&config_system);
    let mut devices   = Devices::new();
    let mut runtime   = Runtime::new();

    startup_logger.log_message("Switching to the curses-based user interface.".to_owned());
    let mut user_interface = match user_interface::UserInterface::new() {
        Some(user_interface) => {
            user_interface
        },
        None => {
            eprintln!("Starting the curses-based user interface failed.");
            return ExitType::AskForEnterOnWindows(1);
        },
    };
    user_interface.consume_startup_logger(startup_logger);
    runtime.power_desired = true;

    match sdl2::init() {
        Ok(sdl2_context) => {

            match sdl2_context.video() {
                Ok(mut video_context) => {

                    match sdl2_context.event_pump() {
                        Ok(mut event_pump) => {

                            sdl2_context.mouse().show_cursor(false);
                            loop {
                                let mut use_hw_accel = config_system.config_items.video_use_hw_accel;
                                let mut use_video = true;
                                runtime.video_system_update = false;

                                if use_hw_accel {
                                    runtime.log_message("Using the hardware accelerated rendering mode.".to_owned());
                                    use_hw_accel = runtime.run_hw_accel(&mut config_system,
                                                                        &mut user_interface,
                                                                        &mut scheduler,
                                                                        &mut devices,
                                                                        &mut memory_system,
                                                                        &mut video_context,
                                                                        &mut event_pump);
                                    if !use_hw_accel {
                                        runtime.log_message("Falling back to using software rendering".to_owned());
                                    }
                                }
                                if !use_hw_accel {
                                    runtime.log_message("Using the software rendering mode.".to_owned());
                                    use_video = runtime.run_sw_render(&mut config_system,
                                                                      &mut user_interface,
                                                                      &mut scheduler,
                                                                      &mut devices,
                                                                      &mut memory_system,
                                                                      &mut video_context,
                                                                      &mut event_pump);

                                    if !use_video {
                                        runtime.log_message("Falling back to not outputting any video.".to_owned());
                                    }
                                }
                                if !use_video {
                                    runtime.log_message("Running without any video output.".to_owned());
                                    runtime.run_without_video(&mut config_system,
                                                              &mut user_interface,
                                                              &mut scheduler,
                                                              &mut devices,
                                                              &mut memory_system);
                                }

                                if runtime.sdl_exit_request {
                                    return ExitType::JustExit(0);
                                }
                                if runtime.curses_exit_request {
                                    return ExitType::AskForEnterOnWindows(0);
                                }
                            }
                        },
                        Err(error) => {
                            runtime.log_message(format!("Failed to initialize the SDL2 event pump: {}.", error));
                        },
                    }
                },
                Err(error) => {
                    runtime.log_message(format!("Failed to initialize the SDL2 video subsystem: {}.", error));
                },
            }
        },
        Err(error) => {
            runtime.log_message(format!("Failed to initialize SDL2: {}.", error));
        }
    }

    runtime.log_message("Falling back to not outputting any video.".to_owned());

    while !runtime.curses_exit_request && !runtime.sdl_exit_request {
        runtime.run_without_video(&mut config_system,
                                  &mut user_interface,
                                  &mut scheduler,
                                  &mut devices,
                                  &mut memory_system);
    }

    ExitType::AskForEnterOnWindows(0)
}
