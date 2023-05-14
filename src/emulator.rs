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
//

use log::{info, warn, error};

use std::path;
use std::sync::mpsc;
use std::thread;
use std::time;

use crate::cassette;
use crate::keyboard;
use crate::sdl_keyboard;
use crate::video;
use crate::machine;
use crate::proj_config;
use crate::util::Sink;
use crate::memory::MemoryChipOps;
use crate::sdl_video;

pub enum EmulatorCassetteCommand {
    Insert { format: cassette::Format, file: String },
    Eject,
    Erase,
    Seek   { position: usize },
    Rewind,
}

pub enum EmulatorConfigCommand {
    List,
    Show   { entry_specifier: String },
    Change { entry_specifier: String, invocation_text: String },
}

// Emulator (logic core) cross-thread commands:
//
pub enum EmulatorCommand {
    PowerOn,
    PowerOff,
    ResetSoft,
    ResetHard,
    Pause,
    Unpause,
    TogglePause,
    Terminate,
    NmiRequest,
    WipeSystemRom,
    LoadSystemRom { path: path::PathBuf, offset: u16 },
    LoadSystemRomDefault,
    WipeSystemRam,
    LoadSystemRam { path: path::PathBuf, offset: u16 },
    SwitchRom(u32),
    CassetteCommand(EmulatorCassetteCommand),
    ConfigCommand(EmulatorConfigCommand),
}

// Emulator (logic core) cross-thread status reports:
//
pub enum EmulatorStatus {
    Created,
    Destroyed,
    TerminateNotification,
    VideoThreadCreated,
    VideoThreadDestroyed,
    PoweredOn,
    PoweredOff,
    Paused,
    NotPaused,
    CpuHalted,
    CpuNotHalted,
}

// Video cross-thread commands:
//
pub enum VideoCommand {
    SetVideoMode {
        windowed_res:          (u32, u32),
        fullscr_res:           (u32, u32),
        desktop_fullscr_mode:  bool,
        use_hw_accel:          bool,
        use_vsync:             bool,
        bg_color:              (u8, u8, u8),
        fg_color:              (u8, u8, u8),
        cg_num:                u32,
    },
    UpdateTextures {
        bg_color:              (u8, u8, u8),
        fg_color:              (u8, u8, u8),
        cg_num:                u32,
    },
    SetFrameDrawing {
        enabled: bool,
        emulation_paused: bool,
    },
    SetWindowedResolution((u32, u32)),
    SetFullscreenResolution((u32, u32), bool),
    SetCyclesPerKeypress(u32),
    DrawFrame(video::VideoFrame),
    Terminate,
}

// Video cross-thread status reports:
//
pub enum VideoStatus {
    Created,
    Destroyed,
    ModeSetStatus(bool),
}

pub struct EmulatorLogicCore {
    machine:              machine::Machine,
    config_system:        proj_config::ConfigSystem,

    cached_cpu_halted:    bool,
    powered_on:           bool,
    paused:               bool,
    exit_request:         bool,
    have_video_thread:    bool,

    selected_rom:         u32,

    video_cmd_tx:         mpsc::Sender<VideoCommand>,
    video_status_rx:      mpsc::Receiver<VideoStatus>,
    status_tx:            mpsc::Sender<EmulatorStatus>,
}

impl EmulatorLogicCore {
    pub fn new(status_tx:       mpsc::Sender<EmulatorStatus>,
               video_cmd_tx:    mpsc::Sender<VideoCommand>,
               video_status_rx: mpsc::Receiver<VideoStatus>,
               config_system:   proj_config::ConfigSystem,
               selected_rom:    u32) -> EmulatorLogicCore {

        let ram_size = config_system.config_items.general_ram_size as u16;
        let rom_choice = EmulatorLogicCore::get_rom_choice(selected_rom, &config_system);
        let lowercase_mod = config_system.config_items.video_lowercase_mod;
        let cassette_file_path = EmulatorLogicCore::get_cassette_path(&config_system);
        let cassette_file_format = config_system.config_items.cassette_file_format;
        let cassette_file_offset = config_system.config_items.cassette_file_offset;
        let cycles_per_video_frame = machine::CPU_HZ / machine::FRAME_RATE;


        let mut emulator = EmulatorLogicCore {
            machine:              machine::Machine::new(ram_size, rom_choice, lowercase_mod, cassette_file_path, cassette_file_format, cassette_file_offset, cycles_per_video_frame),
            config_system,

            cached_cpu_halted:    false,
            powered_on:           false,
            paused:               false,
            exit_request:         false,
            have_video_thread:    false,

            selected_rom,

            video_cmd_tx,
            video_status_rx,
            status_tx,
        };

        emulator.cached_cpu_halted = emulator.machine.cpu.halted;
        emulator.init_video_thread();
        emulator.power_on();
        emulator.send_initial_status();
        emulator
    }
    fn get_rom_choice(selected_rom: u32, config_system: &proj_config::ConfigSystem) -> Option<path::PathBuf> {
        let rom_choice = match selected_rom {
            1 => { config_system.config_items.general_level_1_rom.clone() },
            2 => { config_system.config_items.general_level_2_rom.clone() },
            3 => { config_system.config_items.general_misc_rom.clone() },
            _ => { panic!("Invalid ROM image selected"); }
        };
        match rom_choice {
            Some(filename) => {
                let mut rom_file_path = config_system.config_dir_path.clone();
                rom_file_path.push(filename);
                Some(rom_file_path)
            },
            None => {
                None
            },
        }
    }
    fn get_cassette_path(config_system: &proj_config::ConfigSystem) -> Option<path::PathBuf> {
        match &config_system.config_items.cassette_file {
            Some(filename) => {
                let mut cas_file_path =config_system.config_dir_path.clone();
                cas_file_path.push(filename);
                Some(cas_file_path)
            },
            None => {
                None
            },
        }
    }
    fn send_initial_status(&self) {
        self.status_tx.send(EmulatorStatus::Created).unwrap();

        if self.powered_on {
            self.status_tx.send(EmulatorStatus::PoweredOn).unwrap();
        } else {
            self.status_tx.send(EmulatorStatus::PoweredOff).unwrap();
        }

        if self.paused {
            self.status_tx.send(EmulatorStatus::Paused).unwrap();
        } else {
            self.status_tx.send(EmulatorStatus::NotPaused).unwrap();
        }

        if self.powered_on && !self.paused {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: true, emulation_paused: false }).unwrap();
        } else if self.powered_on && self.paused {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: false, emulation_paused: true }).unwrap();
        } else {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: false, emulation_paused: false }).unwrap();
        }

        if self.machine.cpu.halted {
            self.status_tx.send(EmulatorStatus::CpuHalted).unwrap();
        } else {
            self.status_tx.send(EmulatorStatus::CpuNotHalted).unwrap();
        }
    }
    fn power_on(&mut self) {
        self.machine.power_on();
        self.powered_on = true;

        self.status_tx.send(EmulatorStatus::PoweredOn).unwrap();
        if self.paused {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: false, emulation_paused: true }).unwrap();
        } else {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: true, emulation_paused: false }).unwrap();
        }
        info!("Powered on.");
    }
    fn power_off<ES: Sink<cassette::CassetteEvent>>(&mut self, cassette_event_sink: &mut ES) {

        self.machine.power_off(cassette_event_sink);

        self.powered_on = false;
        self.status_tx.send(EmulatorStatus::PoweredOff).unwrap();

        self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: false, emulation_paused: false }).unwrap();
        info!("Emulator powered off.");
    }
    fn pause(&mut self) {
        self.paused = true;
        if self.powered_on {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: false, emulation_paused: true }).unwrap();
        }
        self.status_tx.send(EmulatorStatus::Paused).unwrap();
        info!("Emulation paused.");
    }
    fn unpause(&mut self) {
        self.paused = false;
        if self.powered_on {
            self.video_cmd_tx.send(VideoCommand::SetFrameDrawing { enabled: true, emulation_paused: false }).unwrap();
        }
        self.status_tx.send(EmulatorStatus::NotPaused).unwrap();
        info!("Emulation unpaused.");
    }
    fn handle_command<ES: Sink<cassette::CassetteEvent>>(&mut self, command: EmulatorCommand, cassette_event_sink: &mut ES) {
        match command {
            EmulatorCommand::PowerOn => {
                if !self.powered_on {
                    self.power_on();
                }
            },
            EmulatorCommand::PowerOff => {
                if self.powered_on {
                    self.power_off(cassette_event_sink);
                }
            },
            EmulatorCommand::ResetSoft => {
                self.machine.cpu.reset();
                info!("System reset performed.");
            },
            EmulatorCommand::ResetHard => {
                self.power_off(cassette_event_sink);
                self.power_on();
                info!("Full reset performed.");
            },
            EmulatorCommand::Pause => {
                if !self.paused {
                    self.pause();
                }
            },
            EmulatorCommand::Unpause => {
                if self.paused {
                    self.unpause();
                }
            },
            EmulatorCommand::TogglePause => {
                if !self.paused {
                    self.pause();
                } else {
                    self.unpause();
                }
            },
            EmulatorCommand::Terminate => {
                self.exit_request = true;
                self.status_tx.send(EmulatorStatus::TerminateNotification).unwrap();
            },
            EmulatorCommand::NmiRequest => {
                self.machine.memory_system.nmi_request = true;
            },
            EmulatorCommand::WipeSystemRom => {
                self.machine.memory_system.rom_chip.wipe();
            },
            EmulatorCommand::LoadSystemRom { path, offset } => {
                self.machine.memory_system.rom_chip.load_from_file(path, offset);
            },
            EmulatorCommand::LoadSystemRomDefault => {
                let rom_choice = EmulatorLogicCore::get_rom_choice(self.selected_rom, &self.config_system);
                self.machine.memory_system.load_system_rom(rom_choice);
            },
            EmulatorCommand::WipeSystemRam => {
                self.machine.memory_system.ram_chip.wipe();
            },
            EmulatorCommand::LoadSystemRam { path, offset } => {
                self.machine.memory_system.ram_chip.load_from_file(path, offset);
            },
            EmulatorCommand::SwitchRom(rom_nr) => {
                if self.selected_rom == rom_nr {
                    info!("ROM {} is already in use, nothing to do.", rom_nr);
                } else {
                    if rom_nr < 1 || rom_nr > 3 {
                        error!("ROM number {} is invalid, valid options are 1 for Level 1 BASIC, 2 for Level 2 basic, and 3 for the miscellaneous rom.", rom_nr);
                    } else {
                        self.selected_rom = rom_nr;
                        let was_powered_on = self.powered_on;

                        if was_powered_on {
                            self.power_off(cassette_event_sink);
                        }

                        let rom_choice = EmulatorLogicCore::get_rom_choice(self.selected_rom, &self.config_system);
                        self.machine.memory_system.load_system_rom(rom_choice);

                        if was_powered_on {
                            self.power_on();
                        }
                    }
                }
            },
            EmulatorCommand::CassetteCommand(sub_command) => {
                match sub_command {
                    EmulatorCassetteCommand::Insert { format, file } => {
                        if file.to_lowercase() == "none" {
                            info!("A filename of `{}' is not allowed, since the config system would understand it as a lack of a cassette.", file);
                        } else {
                            match self.config_system.change_config_entry("cassette_file", format!("= {}", file).as_str()) {
                                Err(error) => {
                                    info!("Failed to set the cassette file in the config system: {}.", error);
                                },
                                Ok(..) => {
                                    let cassette_file_path = EmulatorLogicCore::get_cassette_path(&self.config_system);
                                    if self.machine.devices.cassette.set_cassette_file(cassette_file_path) {

                                        match self.config_system.change_config_entry("cassette_file_format", match format {
                                            cassette::Format::CAS => { "= CAS" },
                                            cassette::Format::CPT => { "= CPT" },
                                        }) {
                                            Err(error) => {
                                                info!("Failed to set the cassette file format in the config system: {}.", error);
                                            },
                                            Ok(..) => {
                                                self.machine.devices.cassette.set_cassette_data_format(self.config_system.config_items.cassette_file_format);
                                                match self.config_system.change_config_entry("cassette_file_offset", "= 0") {
                                                    Err(error) => {
                                                        info!("Failed to set the cassette file offset in the config system: {}.", error);
                                                    },
                                                    Ok(..) => {
                                                        self.machine.devices.cassette.set_cassette_file_offset(self.config_system.config_items.cassette_file_offset);
                                                    }
                                                }
                                            },
                                        }
                                    }
                                },
                            }
                        }
                    },
                    EmulatorCassetteCommand::Eject => {
                        match self.config_system.config_items.cassette_file {
         
                            Some(..) => {
                                match self.config_system.change_config_entry("cassette_file", "= none") {
                                    Err(error) => {
                                        info!("Failed to update the cassette file field in the config system: {}.", error);
                                    },
                                    Ok(..) => {
                                        let cassette_file_path = EmulatorLogicCore::get_cassette_path(&self.config_system);
                                        if self.machine.devices.cassette.set_cassette_file(cassette_file_path) {
                                            info!("Cassette ejected.");
         
                                            match self.config_system.change_config_entry("cassette_file_offset", "= 0") {
                                                Ok(_) => {
                                                    self.machine.devices.cassette.set_cassette_file_offset(self.config_system.config_items.cassette_file_offset);
                                                },
                                                Err(error) => {
                                                    info!("Note: Failed to reset the the file offset to 0: {}.", error);
                                                },
                                            }
                                        }
                                    },
                                }
                            },
                            None => {
                                info!("The cassette drive is already empty.");
                            },
                        }
                    },
                    EmulatorCassetteCommand::Seek { position } => {
                        match self.config_system.change_config_entry("cassette_file_offset", format!("= {}", position).as_str()) {
                            Err(error) => {
                                info!("Failed to set the cassette file offset in the config system: {}.", error);
                            },
                            Ok(..) => {
                                if self.machine.devices.cassette.set_cassette_file_offset(self.config_system.config_items.cassette_file_offset) {
                                    info!("Cassette rewound to position {}.", position);
                                }
                            },
                        }
                    },
                    EmulatorCassetteCommand::Rewind => {
                        match self.config_system.change_config_entry("cassette_file_offset", "= 0") {
                            Err(error) => {
                                info!("Failed to set the cassette file offset in the config system: {}.", error);
                            },
                            Ok(..) => {
                                if self.machine.devices.cassette.set_cassette_file_offset(self.config_system.config_items.cassette_file_offset) {
                                    info!("Cassette rewound back to the beginning.");
                                }
                            },
                        }
                    },
                    EmulatorCassetteCommand::Erase => {
                        self.machine.devices.cassette.erase_cassette();
                    },
                }
            },
            EmulatorCommand::ConfigCommand(sub_command) => {
                match sub_command {
                    EmulatorConfigCommand::List => {
                        let config_entries = match self.config_system.get_config_entry_current_state_all() {
                            Ok(entries) => { entries },
                            Err(error) => {
                                info!("Failed to retrieve a listing of config entries: {}.", error);
                                return;
                            },
                        };
                        info!("Listing of configuration entries:");
                        for config_entry in config_entries {
                            info!("{}", &config_entry);
                        }
                    },
                    EmulatorConfigCommand::Show { entry_specifier } => {
                        let config_entry = match self.config_system.get_config_entry_current_state(&entry_specifier) {
                            Ok(entry) => { entry },
                            Err(error) => {
                                info!("Failed to retrieve the requested config entry: {}.", error);
                                return;
                            },
                        };
                        info!("{}", &config_entry);
                    },
                    EmulatorConfigCommand::Change { entry_specifier, invocation_text } => {
                        match self.config_system.change_config_entry(&entry_specifier, &invocation_text) {
                            Ok(apply_action) => {
                                match apply_action {
                                    proj_config::ConfigChangeApplyAction::RomChange(which) => {
                                        if which == self.selected_rom {
                                            let rom_choice = EmulatorLogicCore::get_rom_choice(self.config_system.config_items.general_default_rom, &self.config_system);
                                            self.machine.memory_system.load_system_rom(rom_choice);
                                        } else {
                                            info!("Configuration updated.");
                                        }
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeRamSize => {
                                        self.machine.memory_system.ram_chip.change_size(self.config_system.config_items.general_ram_size as u16);
                                        info!("Ram size changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::UpdateMsPerKeypress => {
                                        let cycles_per_keypress = (machine::CPU_HZ * self.config_system.config_items.keyboard_ms_per_keypress) / 1_000;

                                        self.video_cmd_tx.send(VideoCommand::SetCyclesPerKeypress(cycles_per_keypress)).unwrap();
                                        info!("Miliseconds per keypress setting updated.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeWindowedResolution => {
                                        self.video_cmd_tx.send(VideoCommand::SetWindowedResolution(self.config_system.config_items.video_windowed_resolution)).unwrap();
                                        info!("Windowed mode resolution changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeFullscreenResolution => {
                                        self.video_cmd_tx.send(VideoCommand::SetFullscreenResolution(self.config_system.config_items.video_fullscreen_resolution, self.config_system.config_items.video_desktop_fullscreen_mode)).unwrap();
                                        info!("Fullscreen mode resolution changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeColor => {
                                        self.video_cmd_tx.send(VideoCommand::UpdateTextures { bg_color: self.config_system.config_items.video_bg_color, fg_color: self.config_system.config_items.video_fg_color, cg_num: self.config_system.config_items.video_character_generator }).unwrap();
                                        info!("Color settings updated.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeHwAccelUsage => {
                                        self.set_video_mode_with_fallback();
                                        info!("Hardware acceleration usage setting changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeVsyncUsage => {
                                        self.set_video_mode_with_fallback();
                                        info!("Vertical synchronization usage setting changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeCharacterGenerator => {
                                        self.video_cmd_tx.send(VideoCommand::UpdateTextures { bg_color: self.config_system.config_items.video_bg_color, fg_color: self.config_system.config_items.video_fg_color, cg_num: self.config_system.config_items.video_character_generator }).unwrap();
                                        info!("Character generator changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::ChangeLowercaseModUsage => {
                                        self.machine.memory_system.vid_mem.update_lowercase_mod(self.config_system.config_items.video_lowercase_mod);
                                        if self.config_system.config_items.video_lowercase_mod {
                                            info!("Lowercase mod enabled. (does not apply to text already in video memory)");
                                        } else {
                                            info!("Lowercase mod disabled. (does not apply to text already in video memory)");
                                        }
                                    },
                                    proj_config::ConfigChangeApplyAction::UpdateCassetteFile => {
                                        let cassette_file_path = EmulatorLogicCore::get_cassette_path(&self.config_system);
                                        self.machine.devices.cassette.set_cassette_file(cassette_file_path);
                                        info!("Cassette file changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::UpdateCassetteFileFormat => {
                                        self.machine.devices.cassette.set_cassette_data_format(self.config_system.config_items.cassette_file_format);
                                        info!("Cassette file data format changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::UpdateCassetteFileOffset => {
                                        self.machine.devices.cassette.set_cassette_file_offset(self.config_system.config_items.cassette_file_offset);
                                        info!("Cassette file offset changed.");
                                    },
                                    proj_config::ConfigChangeApplyAction::UpdateDefaultRomSelection => {
                                        info!("Default system ROM selection changed to ROM {}.", self.config_system.config_items.general_default_rom);
                                        if self.config_system.config_items.general_default_rom != self.selected_rom {
                                            info!("Currently, ROM {} is in use.  To switch to the new default, use the following command: `/machine switch-rom {}'.", self.selected_rom, self.config_system.config_items.general_default_rom);
                                        }
                                    },
                                    proj_config::ConfigChangeApplyAction::AlreadyUpToDate => {
                                        info!("Nothing to change.");
                                    },
                                }
                            },
                            Err(error) => {
                                error!("Failed to perform the requested configuration change: {}.", error);
                                return;
                            },
                        }
                    },
                }
            },
        }
    }
    fn set_video_mode(&mut self, force_hw_accel_off: bool) -> bool {
        self.video_cmd_tx.send(VideoCommand::SetVideoMode {
            windowed_res:          self.config_system.config_items.video_windowed_resolution,
            fullscr_res:           self.config_system.config_items.video_fullscreen_resolution,
            desktop_fullscr_mode:  self.config_system.config_items.video_desktop_fullscreen_mode,
            use_hw_accel:          self.config_system.config_items.video_use_hw_accel && !force_hw_accel_off,
            use_vsync:             self.config_system.config_items.video_use_vsync,
            bg_color:              self.config_system.config_items.video_bg_color,
            fg_color:              self.config_system.config_items.video_fg_color,
            cg_num:                self.config_system.config_items.video_character_generator,
        }).unwrap();

        let status = self.video_status_rx.recv().unwrap();
        match status {
            VideoStatus::Created => {
                self.status_tx.send(EmulatorStatus::VideoThreadCreated).unwrap();
                panic!("Unexpected creation of the SDL2 front-end thread");
            },
            VideoStatus::Destroyed => {
                self.status_tx.send(EmulatorStatus::VideoThreadDestroyed).unwrap();
                panic!("Unexpected termination of the SDL2 front-end thread");
            },
            VideoStatus::ModeSetStatus(status) => {
                status
            },
        }
    }
    fn set_video_mode_with_fallback(&mut self) {
        let mut status = self.set_video_mode(false);

        if !status && self.config_system.config_items.video_use_hw_accel {

            warn!("Falling back to software rendering.");
            status = self.set_video_mode(true);
        }

        if !status {
            panic!("SDL video output not available.");
        }
    }
    fn init_video_thread(&mut self) {

        let status = self.video_status_rx.recv().unwrap();

        match status {
            VideoStatus::Created => {
                self.status_tx.send(EmulatorStatus::VideoThreadCreated).unwrap();
            },
            VideoStatus::Destroyed => {
                self.status_tx.send(EmulatorStatus::VideoThreadDestroyed).unwrap();
                panic!("Unexpected termination of the SDL2 front-end thread");
            },
            VideoStatus::ModeSetStatus(..) => {
                panic!("Received unexpected ModeSetStatus() message from video thread");
            },
        }

        let cycles_per_keypress = (machine::CPU_HZ * self.config_system.config_items.keyboard_ms_per_keypress) / 1_000;

        self.video_cmd_tx.send(VideoCommand::SetCyclesPerKeypress(cycles_per_keypress)).unwrap();
        self.set_video_mode_with_fallback();
        self.have_video_thread = true;
    }
    fn stop_video_thread(&mut self) {

        let started_with_video_thread = self.have_video_thread;

        if self.have_video_thread {
            match self.video_cmd_tx.send(VideoCommand::Terminate) {
                Ok(..) => { },
                Err(..) => {
                    self.have_video_thread = false;
                },
            }
        }

        while self.have_video_thread {
            match self.video_status_rx.recv() {
                Ok(status) => {
                    match status {
                        VideoStatus::Destroyed => {
                            self.have_video_thread = false;
                        },
                        _default => { },
                    }
                },
                Err(..) => {
                    self.have_video_thread = false;
                },
            }
        }
        if started_with_video_thread {
            self.status_tx.send(EmulatorStatus::VideoThreadDestroyed).unwrap();
        }
    }
    pub fn check_for_destroy_status(&self, status: VideoStatus) -> bool {
        match status {
            VideoStatus::Created => {
                warn!("Received unexpected Created message from video thread.");
                false
            },
            VideoStatus::Destroyed => {
                self.status_tx.send(EmulatorStatus::VideoThreadDestroyed).unwrap();
                true
            },
            VideoStatus::ModeSetStatus(..) => {
                warn!("Received unexpected ModeSetStatus() message from video thread.");
                false
            },
        }
    }
    fn handle_cas_event(&mut self, event: cassette::CassetteEvent) {
        match event {
            cassette::CassetteEvent::MotorStarted(_pos) => {
            },
            cassette::CassetteEvent::RecordingStarted => {
            },
            cassette::CassetteEvent::MotorStopped(pos) => {
                match self.config_system.change_config_entry("cassette_file_offset", format!("= {}", pos).as_str()) {
                    Err(error) => {
                        info!("Failed to set the cassette file offset in the config system: {}.", error);
                    },
                    Ok(..) => {
                    },
                }
            },
        }
    }
    pub fn run(&mut self, cmd_rx: &mpsc::Receiver<EmulatorCommand>, kb_rcv: &mpsc::Receiver<keyboard::KeyboardQueueEntry>) {

        let mut frame_begin:     Option<time::Instant>;
        let mut frame_end:       Option<time::Instant>;
        let mut last_frame_ns:   u32;
        let mut residual_ns:     u32;
        let mut frame_cycles:    u32;
        let mut emulated_cycles: u32;

        let mut cassette_event_sink = Vec::new();
        let video_cmd_tx = self.video_cmd_tx.clone();
        let mut video_frame_sink:    MpscSenderSink<VideoCommand> = MpscSenderSink::new(&video_cmd_tx);

        frame_begin = Some(time::Instant::now());

        last_frame_ns = machine::NS_PER_FRAME/3; // Finer granularity than a video frame, for more
                                                // consistent video frame generation.
        emulated_cycles = 0;

        while !self.exit_request {
            // Execute as many machine cycles as we should've executed on the
            // last frame.
            frame_cycles = last_frame_ns / machine::NS_PER_CPU_CYCLE;
            residual_ns  = last_frame_ns % machine::NS_PER_CPU_CYCLE;

            for command in cmd_rx.try_iter() {
                self.handle_command(command, &mut cassette_event_sink);
            }
            if self.have_video_thread {
                for status in self.video_status_rx.try_iter() {
                    let hung_up = self.check_for_destroy_status(status);
                    self.have_video_thread = !hung_up;
                }
            }
            for kb_event in kb_rcv.try_iter() {
                self.machine.devices.keyboard.add_keyboard_event(kb_event);
            }
            for cas_event in cassette_event_sink.drain(..) {
                self.handle_cas_event(cas_event);
            }
            if self.powered_on && !self.paused {
                while emulated_cycles < frame_cycles {
                    emulated_cycles += self.machine.step(&mut cassette_event_sink, &mut video_frame_sink);
                }
                emulated_cycles -= frame_cycles;
            }
            if self.have_video_thread && video_frame_sink.hung_up {
                self.have_video_thread = false;
                self.status_tx.send(EmulatorStatus::VideoThreadDestroyed).unwrap();
            }
            if !self.have_video_thread {
                panic!("Unexpected termination of the SDL2 front-end thread");
            }
            if self.cached_cpu_halted != self.machine.cpu.halted {
                if self.machine.cpu.halted {
                    self.status_tx.send(EmulatorStatus::CpuHalted).unwrap();
                } else {
                    self.status_tx.send(EmulatorStatus::CpuNotHalted).unwrap();
                }
                self.cached_cpu_halted = self.machine.cpu.halted;
            }

            frame_end = Some(time::Instant::now());
            let mut frame_duration = frame_end.unwrap().duration_since(frame_begin.unwrap());

            // If we have time to spare, take a nap.
            let frame_dur_ns = frame_duration.subsec_nanos();
            if frame_duration.as_secs() == 0 &&
                frame_dur_ns < machine::NS_PER_FRAME/3 {

                thread::sleep(time::Duration::new(0, machine::NS_PER_FRAME/3 - frame_dur_ns));
                frame_end = Some(time::Instant::now());
                frame_duration = frame_end.unwrap().duration_since(frame_begin.unwrap());
            }
            if frame_duration.as_secs() == 0 {
                last_frame_ns = frame_duration.subsec_nanos();
            } else {
                // Throttle / slow down the emulation in case a frame
                // lasted longer than a second.
                last_frame_ns = 1_000_000_000;
            }

            // Take care of the remaining time from the frame before this one
            // that was too short to execute any cycles:
            last_frame_ns += residual_ns;

            // Our end is someone else's beginning.
            frame_begin = frame_end;
        }
    }
}

impl Drop for EmulatorLogicCore {
    fn drop(&mut self) {
        self.stop_video_thread();
        self.status_tx.send(EmulatorStatus::Destroyed).unwrap();
    }
}

struct SdlWindowState {
    canvas:          sdl2::render::Canvas<sdl2::video::Window>,
    windowed_res:    (u32, u32),
    fullscr_res:     (u32, u32),
    fullscreen_mode: bool,
    fscr_mode_dsktp: bool,
}

pub struct EmulatorSdlFrontend {

    sdl2_main_ctxt:  sdl2::Sdl,
    sdl2_video_ctxt: sdl2::VideoSubsystem,
    sdl2_event_pump: sdl2::EventPump,
    sdl2_keyboard:   sdl_keyboard::SdlKeyboard,

    frame_draw:      bool,
    emu_paused:      bool,
    cur_frame_used:  bool,
    current_frame:   Option<video::VideoFrame>,
    delayed_command: Option<VideoCommand>,

    kb_tx:           mpsc::Sender<keyboard::KeyboardQueueEntry>,
    lc_cmd_tx:       mpsc::Sender<EmulatorCommand>,
    status_tx:       mpsc::Sender<VideoStatus>,
}

impl EmulatorSdlFrontend {
    pub fn new(kb_tx: mpsc::Sender<keyboard::KeyboardQueueEntry>, lc_cmd_tx: mpsc::Sender<EmulatorCommand>, status_tx: mpsc::Sender<VideoStatus>) -> EmulatorSdlFrontend {

        let main_ctxt = match sdl2::init() {
            Ok(context) => { context },
            Err(error) => {
                panic!("Failed to initialize SDL2: {}", error);
            },
        };
        let video_ctxt = match main_ctxt.video() {
            Ok(context) => { context },
            Err(error) => {
                panic!("Failed to initialize the SDL2 video subsystem: {}", error);
            },
        };
        let event_pump = match main_ctxt.event_pump() {
            Ok(context) => { context },
            Err(error) => {
                panic!("Failed to initialize the SDL2 event pump: {}", error);
            },
        };
        main_ctxt.mouse().show_cursor(false);
        status_tx.send(VideoStatus::Created).unwrap();

        EmulatorSdlFrontend {
            sdl2_main_ctxt:  main_ctxt,
            sdl2_video_ctxt: video_ctxt,
            sdl2_event_pump: event_pump,
            sdl2_keyboard:   sdl_keyboard::SdlKeyboard::new(0),
            frame_draw:      false,
            emu_paused:      false,
            cur_frame_used:  false,
            current_frame:   None,
            delayed_command: None,
            kb_tx,
            lc_cmd_tx,
            status_tx,
        }
    }
    fn create_draw_ctxt(&mut self,
                        windowed_res:          (u32, u32),
                        fullscr_res:           (u32, u32),
                        desktop_fullscr_mode:  bool,
                        use_hw_accel:          bool,
                        use_vsync:             bool) -> Option<(SdlWindowState, sdl2::render::TextureCreator<sdl2::video::WindowContext>)> {

        let (width, height) = windowed_res;
        let mut window_builder = self.sdl2_video_ctxt.window("TRS-80 Model I Emulator", width, height);

        let window = match window_builder.position_centered().build() {
            Ok(window) => { window },
            Err(error) => {
                error!("Failed to create a window for the SDL2 front-end: {}.", error);
                return None;
            },
        };

        let mut canvas = match if use_hw_accel {
            if use_vsync {
                window.into_canvas().accelerated().present_vsync().build()
            } else {
                window.into_canvas().accelerated().build()
            }
        } else {
            if use_vsync {
                window.into_canvas().software().present_vsync().build()
            } else {
                window.into_canvas().software().build()
            }
        } {
            Ok(canvas) => { canvas },
            Err(error) => {
                error!("Failed to create a {} SDL2 rendering context: {}.", if use_hw_accel { "hardware accelerated" } else { "software" }, error);
                return None;
            },
        };
        match canvas.set_logical_size(video::SCREEN_WIDTH, video::SCREEN_HEIGHT) {
            Ok(..) => { () },
            Err(error) => {
                error!("Failed to set the SDL2 renderer's logical size: {}.", error);
                return None;
            },
        }

        let texture_creator = canvas.texture_creator();

        Some((SdlWindowState {
            canvas,
            windowed_res,
            fullscr_res,
            fullscreen_mode: false,
            fscr_mode_dsktp: desktop_fullscr_mode,
        }, texture_creator))
    }
    fn handle_video_cmd_toplevel(&mut self, wnd_state: &mut SdlWindowState, cmd: VideoCommand, terminate_thread: &mut bool) -> bool
    {
        *terminate_thread = false;
        match cmd {
            VideoCommand::SetFrameDrawing{ enabled, emulation_paused } => {
                self.frame_draw = enabled;
                self.emu_paused = emulation_paused;
                false
            },
            VideoCommand::DrawFrame(frame) => {
                self.current_frame = Some(frame);
                self.cur_frame_used = false;
                false
            },
            VideoCommand::SetCyclesPerKeypress(cycles_per_keypress) => {
                self.sdl2_keyboard.set_cycles_per_keypress(cycles_per_keypress);
                false
            }
            VideoCommand::Terminate => {
                *terminate_thread = true;
                true
            },
            VideoCommand::UpdateTextures { bg_color, fg_color, cg_num } => {
                self.delayed_command = Some(VideoCommand::UpdateTextures { bg_color, fg_color, cg_num });
                true
            },
            VideoCommand::SetWindowedResolution((width, height)) => {
                wnd_state.windowed_res = (width, height);
                if !wnd_state.fullscreen_mode {
                    let window = wnd_state.canvas.window_mut();
                    window.set_size(width, height).unwrap();
                }
                false
            },
            VideoCommand::SetFullscreenResolution((width, height), fscr_mode_dsktp) => {
                self.handle_fullscr_res_change(wnd_state, width, height, fscr_mode_dsktp);
                false
            },
            VideoCommand::SetVideoMode { windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync, bg_color, fg_color, cg_num } => {

                self.delayed_command = Some(VideoCommand::SetVideoMode{ windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync, bg_color, fg_color, cg_num });
                true
            },
        }
    }
    fn handle_sdl_events(&mut self, wnd_state: &mut SdlWindowState, capture_kbd: bool) {

        let mut fullscreen_toggle = false;
        self.sdl2_keyboard.handle_events(&self.lc_cmd_tx, &mut self.sdl2_event_pump, &mut fullscreen_toggle, &self.kb_tx, capture_kbd);

        if fullscreen_toggle {
            let window = wnd_state.canvas.window_mut();

            if !wnd_state.fullscreen_mode {

                if !wnd_state.fscr_mode_dsktp {
                    let (width, height) = wnd_state.fullscr_res;
                    window.set_size(width, height).unwrap();
                    window.set_fullscreen(sdl2::video::FullscreenType::True).unwrap();
                } else {
                    window.set_fullscreen(sdl2::video::FullscreenType::Desktop).unwrap();
                }
                wnd_state.fullscreen_mode = true;

            } else {
                let (width, height) = wnd_state.windowed_res;
                window.set_fullscreen(sdl2::video::FullscreenType::Off).unwrap();
                window.set_size(width, height).unwrap();
                window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);

                wnd_state.fullscreen_mode = false;
            }
        }
    }
    fn handle_fullscr_res_change(&mut self, wnd_state: &mut SdlWindowState, width: u32, height: u32, fscr_mode_dsktp: bool) {

        let window = wnd_state.canvas.window_mut();
        if wnd_state.fullscreen_mode {

            if wnd_state.fscr_mode_dsktp != fscr_mode_dsktp {

                let (width_w, height_w) = wnd_state.windowed_res;
                window.set_fullscreen(sdl2::video::FullscreenType::Off).unwrap();
                window.set_size(width_w, height_w).unwrap();
                window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);

                if !fscr_mode_dsktp {
                    window.set_size(width, height).unwrap();
                    window.set_fullscreen(sdl2::video::FullscreenType::True).unwrap();
                } else {
                    window.set_fullscreen(sdl2::video::FullscreenType::Desktop).unwrap();
                }

            } else if !wnd_state.fscr_mode_dsktp {

                let window = wnd_state.canvas.window_mut();
                window.set_size(width, height).unwrap();
            }
        }
        wnd_state.fullscr_res = (width, height);
        wnd_state.fscr_mode_dsktp = fscr_mode_dsktp;
    }
    fn run_with_textures(&mut self,
                         cmd_rx:    &mpsc::Receiver<VideoCommand>,
                         wnd_state: &mut SdlWindowState,
                         txt_creat: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
                         bg_color:  (u8, u8, u8),
                         fg_color:  (u8, u8, u8),
                         cg_num:    u32) -> bool {

        let (narrow_glyphs, wide_glyphs) = sdl_video::generate_glyph_textures(bg_color, fg_color, cg_num, txt_creat);
        let mut sticky_clear = false;

        loop {
            self.handle_sdl_events(wnd_state, self.frame_draw);
            if self.frame_draw {
                for cmd in cmd_rx.try_iter() {

                    let mut terminate_thread = false;
                    let exit_func = self.handle_video_cmd_toplevel(wnd_state, cmd, &mut terminate_thread);

                    if terminate_thread {
                        return false;
                    } else if exit_func {
                        return true;
                    }
                }
                while self.frame_draw && match self.current_frame { Some(..) => { self.cur_frame_used }, None => { true } } {

                    let cmd = cmd_rx.recv().unwrap();

                    let mut terminate_thread = false;
                    let exit_func = self.handle_video_cmd_toplevel(wnd_state, cmd, &mut terminate_thread);

                    if terminate_thread {
                        return false;
                    } else if exit_func {
                        return true;
                    }
                }
                if self.frame_draw {

                    match &self.current_frame {
                        Some(frame) => {
                            sdl_video::render(&mut wnd_state.canvas, &narrow_glyphs, &wide_glyphs, frame);
                        },
                        None => {
                            // This point should be impossible to reach.
                            //
                            let (bg_red, bg_green, bg_blue) = bg_color;
                            wnd_state.canvas.set_draw_color(sdl2::pixels::Color::RGB(bg_red, bg_green, bg_blue));
                            wnd_state.canvas.clear();
                        },
                    }
                    self.cur_frame_used = true;
                }
                sticky_clear = false;

            } else {

                let mut delayed_command: Option<VideoCommand> = None;
                std::mem::swap(&mut delayed_command, &mut self.delayed_command);

                let command_opt = match delayed_command {
                    Some(cmd) => { Some(cmd) },
                    None => {
                        match cmd_rx.try_recv() {
                            Ok(cmd) => { Some(cmd) },
                            Err(error) => {
                                match error {
                                    mpsc::TryRecvError::Empty => {
                                        // When frame drawing is disabled, and there are no
                                        // messages to be processed, run the loop at a reduced
                                        // frame rate (~10 fps should do just fine):
                                        thread::sleep(time::Duration::new(0, 100_000_000));
                                        None
                                    },
                                    mpsc::TryRecvError::Disconnected => {
                                        panic!("Video command transmitter disconnected.");
                                    },
                                }
                            },
                        }
                    },
                };
                match command_opt {
                    Some(cmd) => {

                        let mut terminate_thread = false;
                        let exit_func = self.handle_video_cmd_toplevel(wnd_state, cmd, &mut terminate_thread);

                        if terminate_thread {
                            return false;
                        } else if exit_func {
                            return true;
                        }
                    },
                    None => { },
                }

                // Was the machine powered down?  Clear the screen.
                if !self.emu_paused || sticky_clear {
                    let (bg_red, bg_green, bg_blue) = bg_color;
                    wnd_state.canvas.set_draw_color(sdl2::pixels::Color::RGB(bg_red, bg_green, bg_blue));
                    wnd_state.canvas.clear();
                    sticky_clear = true;

                } else {

                    // Otherwise, draw the previous frame, if any.
                    match &self.current_frame {
                        Some(frame) => {
                            sdl_video::render(&mut wnd_state.canvas, &narrow_glyphs, &wide_glyphs, frame);
                        },
                        None => {
                            let (bg_red, bg_green, bg_blue) = bg_color;
                            wnd_state.canvas.set_draw_color(sdl2::pixels::Color::RGB(bg_red, bg_green, bg_blue));
                            wnd_state.canvas.clear();
                        },
                    }
                }
                wnd_state.canvas.present();
            }
        }
    }
    fn run_in_mode(&mut self,
                   cmd_rx:                &mpsc::Receiver<VideoCommand>,
                   windowed_res:          (u32, u32),
                   fullscr_res:           (u32, u32),
                   desktop_fullscr_mode:  bool,
                   use_hw_accel:          bool,
                   use_vsync:             bool) -> bool {

        let (mut wnd_state, txt_creat) = match self.create_draw_ctxt(windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync) {
            Some((ctxt, creat)) => {
                self.status_tx.send(VideoStatus::ModeSetStatus(true)).unwrap();
                (ctxt, creat)
            },
            None => {
                self.status_tx.send(VideoStatus::ModeSetStatus(false)).unwrap();
                return true; // Don't stop the video thread if mode setting failed.
            },
        };
        loop {
            self.handle_sdl_events(&mut wnd_state, self.frame_draw);

            let mut delayed_command: Option<VideoCommand> = None;
            std::mem::swap(&mut delayed_command, &mut self.delayed_command);

            match delayed_command.unwrap_or_else(|| cmd_rx.recv().unwrap()) {
                VideoCommand::SetFrameDrawing{ enabled, emulation_paused } => {
                    self.frame_draw = enabled;
                    self.emu_paused = emulation_paused;
                },
                VideoCommand::DrawFrame(frame) => {
                    self.current_frame = Some(frame);
                    self.cur_frame_used = false;
                },
                VideoCommand::SetCyclesPerKeypress(cycles_per_keypress) => {
                    self.sdl2_keyboard.set_cycles_per_keypress(cycles_per_keypress);
                }
                VideoCommand::Terminate => {
                    return false;
                },
                VideoCommand::UpdateTextures { bg_color, fg_color, cg_num } => {
                    if !self.run_with_textures(cmd_rx, &mut wnd_state, &txt_creat, bg_color, fg_color, cg_num) {
                        return false;
                    }
                },
                VideoCommand::SetWindowedResolution((width, height)) => {
                    wnd_state.windowed_res = (width, height);
                    if !wnd_state.fullscreen_mode {
                        let window = wnd_state.canvas.window_mut();
                        window.set_size(width, height).unwrap();
                    }
                },
                VideoCommand::SetFullscreenResolution((width, height), fscr_mode_dsktp) => {
                    self.handle_fullscr_res_change(&mut wnd_state, width, height, fscr_mode_dsktp);
                },
                VideoCommand::SetVideoMode { windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync, bg_color, fg_color, cg_num } => {

                    self.delayed_command = Some(VideoCommand::SetVideoMode{ windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync, bg_color, fg_color, cg_num });
                    return true;
                },
            }
        }
    }
    pub fn run(&mut self, cmd_rx: &mpsc::Receiver<VideoCommand>) {

        loop {
            let mut delayed_command: Option<VideoCommand> = None;
            std::mem::swap(&mut delayed_command, &mut self.delayed_command);

            match delayed_command.unwrap_or_else(|| cmd_rx.recv().unwrap()) {
                VideoCommand::SetFrameDrawing{ enabled, emulation_paused } => {
                    self.frame_draw = enabled;
                    self.emu_paused = emulation_paused;
                },
                VideoCommand::DrawFrame(frame) => {
                    self.current_frame = Some(frame);
                    self.cur_frame_used = false;
                },
                VideoCommand::SetCyclesPerKeypress(cycles_per_keypress) => {
                    self.sdl2_keyboard.set_cycles_per_keypress(cycles_per_keypress);
                }
                VideoCommand::Terminate => {
                    return;
                },
                VideoCommand::UpdateTextures { .. } => {
                },
                VideoCommand::SetWindowedResolution(..) => {
                },
                VideoCommand::SetFullscreenResolution(..) => {
                },
                VideoCommand::SetVideoMode { windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync, bg_color, fg_color, cg_num } => {

                    self.delayed_command = Some(VideoCommand::UpdateTextures { bg_color, fg_color, cg_num });
                    if !self.run_in_mode(cmd_rx, windowed_res, fullscr_res, desktop_fullscr_mode, use_hw_accel, use_vsync) {
                        return;
                    }
                },
            }
        }
    }
}

impl<T> Sink<T> for Vec<T> {
    fn push(&mut self, value: T) {
        self.push(value);
    }
}

struct MpscSenderSink<'a, T> {
    pub sender: &'a mpsc::Sender<T>,
    pub hung_up: bool,
}

impl<'a, T> MpscSenderSink<'a, T> {
    pub fn new(sender: &'a mpsc::Sender<T>) -> MpscSenderSink<T> {

        MpscSenderSink {
            sender,
            hung_up: false,
        }
    }
}


impl Sink<video::VideoFrame> for MpscSenderSink<'_, VideoCommand> {

    fn push(&mut self, value: video::VideoFrame) {

        if !self.hung_up {

            match self.sender.send(VideoCommand::DrawFrame(value)) {
                Ok(..) => { },
                Err(..) => {
                    self.hung_up = true;
                },
            }
        }
    }
}

impl Drop for EmulatorSdlFrontend {
    fn drop(&mut self) {
        match self.status_tx.send(VideoStatus::Destroyed) {
            Ok(..) => { },
            Err(..) => { }, // Ignore error to prevent double panic.
        }
    }
}
