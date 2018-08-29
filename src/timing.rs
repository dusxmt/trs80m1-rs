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

use emulator;
use memory;
use proj_config;
use time;

// Timing description:
pub const MASTER_HZ:          u32 = 10_644_480;
pub const FRAME_RATE:         u32 = MASTER_HZ     / 177_408;
pub const NS_PER_FRAME:       u32 = 1_000_000_000 / FRAME_RATE;
pub const CPU_HZ:             u32 = MASTER_HZ     / 6;
pub const NS_PER_CPU_CYCLE:   u32 = 1_000_000_000 / CPU_HZ;


// The following is a timer which manages frames.
//
//
// At the start of a frame, the frame_cycles() method is to be invoked, which
// returns the amount of cycles that correspond to the length of the previously
// performed frame.
//
// Once all work within the frame is done, frame_next() is to be invoked, which
// performs a wait if there is spare time, and prepares the cycle count info
// for the next frame.
//
pub struct FrameTimer {
    ns_per_frame:    u32,
    ns_per_cycle:    u32,

    frame_begin:     std_time::Duration,
    frame_cycles:    u32,
    residual_ns:     u32,
}

impl FrameTimer {
    pub fn new(ns_per_frame: u32, ns_per_cycle: u32) -> FrameTimer {
        let stamp_now = time::get_time();

        FrameTimer {
            ns_per_frame:   ns_per_frame,
            ns_per_cycle:   ns_per_cycle,

            frame_begin:    std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32),

            frame_cycles:   ns_per_frame / ns_per_cycle,
            residual_ns:    ns_per_frame % ns_per_cycle,
        }
    }
    pub fn set_ns_per_frame(&mut self, ns_per_frame: u32) {
        self.ns_per_frame = ns_per_frame;
    }
    pub fn frame_cycles(&mut self) -> u32 {
        self.frame_cycles
    }
    pub fn frame_next(&mut self) {
        let stamp_now = time::get_time();
        let mut frame_end = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);
        let mut frame_duration = frame_end - self.frame_begin;

        // If we have time to spare, take a nap.
        let frame_dur_ns = frame_duration.subsec_nanos();
        if frame_duration.as_secs() == 0 && frame_dur_ns < self.ns_per_frame {

            thread::sleep(std_time::Duration::new(0, self.ns_per_frame - frame_dur_ns));
            let stamp_now = time::get_time();
            frame_end = std_time::Duration::new(stamp_now.sec as u64, stamp_now.nsec as u32);
            frame_duration = frame_end - self.frame_begin;
        }

        let mut last_frame_ns = if frame_duration.as_secs() == 0 {
            frame_duration.subsec_nanos()
        } else {
            // In case the frame lasted longer than a second... pretend
            // that it didn't :3
            1_000_000_000
        };

        // Take care of the remaining time from the frame before this one
        // that was too short to execute any cycles:
        last_frame_ns += self.residual_ns;

        self.frame_cycles = last_frame_ns / self.ns_per_cycle;
        self.residual_ns  = last_frame_ns % self.ns_per_cycle;

        // Our end is someone else's beginning.
        self.frame_begin = frame_end;
    }
}


// Execution scheduler:
//
// The following structure handles the timely invocation of the different
// devices present within the emulator.
//
// The current implementation is quite a bit simplified from what I'd eventually
// like to have, but since we only have one processor and one IO device
// (keyboard), it is adequate.
//
pub struct Scheduler {
    // Keyboard timing information:
    cycles_per_keypress:  u32,
    cycles_since_last:    u32,
}

impl Scheduler {
    pub fn new(config_system: &proj_config::ConfigSystem) -> Scheduler {
        let mut scheduler = Scheduler {
                                cycles_per_keypress:  0,
                                cycles_since_last:    0,
                            };
        scheduler.update (config_system);

        scheduler
    }
    pub fn update(&mut self, config_system: &proj_config::ConfigSystem) {
        self.cycles_per_keypress = (CPU_HZ * config_system.config_items.keyboard_ms_per_keypress) / 1000;
        self.cycles_since_last   = 0;
    }
    pub fn perform_cycles(&mut self,
                          cycles_to_exec:  u32,
                          devices:         &mut emulator::Devices,
                          memory_system:   &mut memory::MemorySystem) {

        let mut needed_cycles = cycles_to_exec;

        while (self.cycles_since_last + needed_cycles) > self.cycles_per_keypress {
            let to_exec = self.cycles_per_keypress - self.cycles_since_last;

            devices.cpu.exec(to_exec, memory_system);
            devices.keyboard.update(memory_system);

            self.cycles_since_last = 0;
            needed_cycles -= to_exec;
        }
        if needed_cycles > 0 {
            devices.cpu.exec(needed_cycles, memory_system);
            self.cycles_since_last += needed_cycles;
        }
    }
}
