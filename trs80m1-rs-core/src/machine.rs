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
//

use log::{info, warn, error};

use std::path;

use crate::z80::cpu;
use crate::cassette;
use crate::keyboard;
use crate::video;
use crate::memory;
use crate::util::Sink;

// Timing description:
pub const MASTER_HZ:          u32 = 10_644_480;
pub const FRAME_RATE:         u32 = MASTER_HZ     / 177_408;
pub const NS_PER_FRAME:       u32 = 1_000_000_000 / FRAME_RATE;
pub const CPU_HZ:             u32 = MASTER_HZ     / 6;
pub const NS_PER_CPU_CYCLE:   u32 = 1_000_000_000 / CPU_HZ;

pub struct Devices {
    pub cassette: cassette::CassetteRecorder,
    pub keyboard: keyboard::KeyboardQueue,
    pub video:    video::Video,
}

impl Devices {
    fn new(cassette_file_path: Option<path::PathBuf>,
           cassette_file_format: cassette::Format,
           cassette_file_offset: usize,
           cycles_per_video_frame: u32) -> Devices {
        Devices {
            cassette: cassette::CassetteRecorder::new(cassette_file_path, cassette_file_format, cassette_file_offset),
            keyboard: keyboard::KeyboardQueue::new(),
            video:    video::Video::new(cycles_per_video_frame),
        }
    }
    fn power_off<ES: Sink<cassette::CassetteEvent>>(&mut self, memory_system: &mut memory::MemorySystem, cassette_event_sink: &mut ES) {
        self.cassette.power_off(&mut memory_system.cas_rec, cassette_event_sink);
        self.keyboard.power_off(&mut memory_system.kbd_mem);
        self.video.power_off(&mut memory_system.vid_mem);
    }
    fn tick<ES: Sink<cassette::CassetteEvent>, VS: Sink<video::VideoFrame>>(&mut self, memory_system: &mut memory::MemorySystem, cpu_cycles: u32, cassette_event_sink: &mut ES, video_frame_sink: &mut VS) {
        self.cassette.tick(&mut memory_system.cas_rec, cpu_cycles, cassette_event_sink);
        self.keyboard.tick(&mut memory_system.kbd_mem, cpu_cycles);
        self.video.tick(&mut memory_system.vid_mem, cpu_cycles, video_frame_sink);
    }
}

pub struct Machine {
    pub cpu:               cpu::CPU,
    pub memory_system:     memory::MemorySystem,
    pub devices:           Devices,
}

impl Machine {

    pub fn new(ram_size: u16,
               rom_choice: Option<path::PathBuf>,
               lowercase_mod: bool,
               cassette_file_path: Option<path::PathBuf>,
               cassette_file_format: cassette::Format,
               cassette_file_offset: usize,
               cycles_per_video_frame: u32,
               ) -> Machine {

        Machine {
            cpu: cpu::CPU::new(),
            memory_system: memory::MemorySystem::new(ram_size, rom_choice, lowercase_mod),
            devices: Devices::new(cassette_file_path, cassette_file_format, cassette_file_offset, cycles_per_video_frame),
        }
    }
    pub fn power_on(&mut self) {
        self.cpu.full_reset();
    }
    pub fn power_off<ES: Sink<cassette::CassetteEvent>>(&mut self, cassette_event_sink: &mut ES) {

        self.cpu.power_off();
        self.devices.power_off(&mut self.memory_system, cassette_event_sink);
        self.memory_system.power_off();
    }
    pub fn step<ES: Sink<cassette::CassetteEvent>, VS: Sink<video::VideoFrame>>(&mut self, cassette_event_sink: &mut ES, video_frame_sink: &mut VS) -> u32 {

        let cpu_cycles = self.cpu.step(&mut self.memory_system);
        self.devices.tick(&mut self.memory_system, cpu_cycles, cassette_event_sink, video_frame_sink);

        cpu_cycles
    }
}
