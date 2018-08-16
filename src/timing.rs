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

use time;


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
