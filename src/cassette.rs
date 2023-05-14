// The cassette emulation implementation provided in this file is based heavily
// on that present in the xtrs emulator, in the `trs_cassette.c` file.

/*
 * Copyright (C) 1992 Clarendon Hill Software.
 *
 * Permission is granted to any individual or institution to use, copy,
 * or redistribute this software, provided this copyright notice is retained.
 *
 * This software is provided "as is" without any expressed or implied
 * warranty.  If this software brings on any sort of damage -- physical,
 * monetary, emotional, or brain -- too bad.  You've got no one to blame
 * but yourself.
 *
 * The software may be modified for your own purposes, but modified versions
 * must retain this notice.
 */

/*
   Modified by Timothy Mann, 1996 and later
   $Id: trs_cassette.c,v 1.26 2008/06/26 04:39:56 mann Exp $
 */

/*
   Modified by Marek Benc, 2017, 2018, 2023, to adapt it for the rust-based
   trs80m1-rs emulator.
 */

use log::{info, warn, error};

use std::path;
use std::fs;
use std::io::Read;
use std::io::Write;

use crate::memory;
use crate::machine;
use crate::util::Sink;


const CPU_MHZ:    f32 = (machine::CPU_HZ as f32) / (1_000_000 as f32);
const DETECT_250: f32 = 1200.0;   // For level 1 input routine detection.


#[derive(Copy, Clone, PartialEq, Debug)] // For the config system.
pub enum Format {
    CAS,  // Recovered bit/byte stream.
    CPT,  // Cassette pulse train w/ exact machine.
}

pub enum CassetteEvent {
    MotorStarted(usize),
    MotorStopped(usize),
    RecordingStarted,
}

#[derive(Copy, Clone, PartialEq)]
enum State {
    AudioOut,          // Motor is not running, tape recording output redirected to speakers.
    RecModeUncertain,  // Motor is running, but not sure if for playback or recording.
    Playback,          // Motor is running, reading tape.
    Recording,         // Motor is running, recording to tape.
}

#[derive(PartialEq)]
enum Speed {
    S500,
    S250,
}

#[derive(PartialEq)]
enum OutVal {
    Level(i8),
    Flush,
}

const NOISE_FLOOR: i32 = 64;

// Pulse shapes for conversion from .cas on input:
struct PulseShape {
    delta_us: i32,
    next_lvl: i8,
}

static S500_SHAPE_ZERO: [PulseShape; 5] =
  [ PulseShape { delta_us: 0,    next_lvl: 1  },
    PulseShape { delta_us: 128,  next_lvl: 2  },
    PulseShape { delta_us: 128,  next_lvl: 0  },
    PulseShape { delta_us: 1757, next_lvl: 0  },    /* 1871 after 8th bit */
    PulseShape { delta_us: -1,   next_lvl: -1 } ];

static S500_SHAPE_ONE: [PulseShape; 8] =
  [ PulseShape { delta_us: 0,    next_lvl: 1  },
    PulseShape { delta_us: 128,  next_lvl: 2  },
    PulseShape { delta_us: 128,  next_lvl: 0  },
    PulseShape { delta_us: 748,  next_lvl: 1  },
    PulseShape { delta_us: 128,  next_lvl: 2  },
    PulseShape { delta_us: 128,  next_lvl: 0  },
    PulseShape { delta_us: 748,  next_lvl: 0  },    /* 860 after 8th bit; 1894 after a5 sync */
    PulseShape { delta_us: -1,   next_lvl: -1 } ];

static S250_SHAPE_ZERO: [PulseShape; 5] =
  [ PulseShape { delta_us: 0,    next_lvl: 1  },
    PulseShape { delta_us: 125,  next_lvl: 2  },
    PulseShape { delta_us: 125,  next_lvl: 0  },
    PulseShape { delta_us: 3568, next_lvl: 0  },
    PulseShape { delta_us: -1,   next_lvl: -1 } ];

static S250_SHAPE_ONE: [PulseShape; 8] =
  [ PulseShape { delta_us: 0,    next_lvl: 1  },
    PulseShape { delta_us: 128,  next_lvl: 2  },
    PulseShape { delta_us: 128,  next_lvl: 0  },
    PulseShape { delta_us: 1673, next_lvl: 1  },
    PulseShape { delta_us: 128,  next_lvl: 2  },
    PulseShape { delta_us: 128,  next_lvl: 0  },
    PulseShape { delta_us: 1673, next_lvl: 0  },
    PulseShape { delta_us: -1,   next_lvl: -1 } ];

// States and thresholds for conversion to .cas on output:
const   ST_INITIAL:    usize = 0;
const   ST_500GOTCLK:  usize = 1;
const   ST_500GOTDAT:  usize = 2;
//const ST_1500:       usize = 3;
const   ST_250:        usize = 4;
const   ST_250GOTCLK:  usize = 5;
const   ST_250GOTDAT:  usize = 6;
const   ST_500THRESH:  f32   = 1250.0; // us threshold between 0 and 1
//const ST_1500THRESH: f32   = 282.0;  // us threshold between 1 and 0
const   ST_250THRESH:  f32   = 2500.0; // us threshold between 0 and 1

// Some of the constants are commented out, as the Model I doesn't have a
// 1500 baud cassette port.


pub struct CassetteIO {
    read_flag:   bool,
    write_flag:  bool,
    in_latch:    bool,
    out_latch:   u8,
    motor_req:   bool,
}

impl CassetteIO {
    pub fn new() -> CassetteIO {

        CassetteIO {
            read_flag:  false,
            write_flag: false,
            in_latch:   false,
            out_latch:  0,
            motor_req:  false,
        }
    }
}

impl memory::PeripheralIO for CassetteIO {
    fn peripheral_read_byte(&mut self, offset: u16) -> u8 {

        if offset != 0 {
            panic!("Failed read: Port offset 0x{:02X} is invalid for the cassette recorder", offset);
        }

        self.read_flag = true;
        if self.in_latch { 0b1111_1111 } else { 0b0111_1111 }
    }
    fn peripheral_write_byte(&mut self, offset: u16, val: u8) {

        if offset != 0 {
            panic!("Failed write: Port offset 0x{:02X} is invalid for the cassette recorder", offset);
        }

        self.motor_req  = (val & 0b0000_0100) != 0;
        self.out_latch  =  val & 0x03;
        self.in_latch   = false;                    // Write accesses reset the input latch.
        self.write_flag = true;
    }
}

pub struct CassetteRecorder {

    state:            State,
    motor:            bool,
    cas_path:         Option<path::PathBuf>,
    io_buffer:        Option<Vec<u8>>,
    io_buffer_iter:   usize,
    iter_backup:      usize,
    data_format:      Format,
    avg:              f32,
    env:              f32,
    noise_floor:      i32,
    sample_rate:      i32,

    // For bit-level emulation:
    cpu_delta:        u32,
    have_read_out_1:  bool,
    read_out_1_delta: u32,
    latch_lvl:        i8,
    next_in_lvl:      i8,
    transitions_out:  i32,
    in_trans_delta:   u32,
    roundoff_error:   f32,

    // For bit/byte conversion (.cas file i/o):
    cas_pulse_state:  usize,
    cas_speed:        Speed,
    cas_byte:         i32,
    cas_bit_num:      i32,
}

impl CassetteRecorder {
    pub fn new(cassette_file_path: Option<path::PathBuf>, cassette_file_format: Format, cassette_file_offset: usize) -> CassetteRecorder {

        let mut recorder = CassetteRecorder {

            state:            State::AudioOut,
            motor:            false,
            cas_path:         None,
            io_buffer:        None,
            io_buffer_iter:   cassette_file_offset,
            iter_backup:      0,
            data_format:      cassette_file_format,
            avg:              0.0,
            env:              0.0,
            noise_floor:      0,
            sample_rate:      0,

            // For bit-level emulation:
            cpu_delta:        0,
            have_read_out_1:  false,
            read_out_1_delta: 0,
            latch_lvl:        0,
            next_in_lvl:      0,
            transitions_out:  0,
            in_trans_delta:   0,
            roundoff_error:   0.0,

            // For bit/byte conversion (.cas file i/o):
            cas_pulse_state:  0,
            cas_speed:        Speed::S500,
            cas_byte:         0,
            cas_bit_num:      0,
        };
        recorder.set_cassette_file(cassette_file_path);
        info!("Created the cassette recorder.");

        recorder
    }
    pub fn tick<ES: Sink<CassetteEvent>>(&mut self, io: &mut CassetteIO, cycles: u32, event_sink: &mut ES) {

        if io.write_flag && io.read_flag {
            panic!("Cassette drive I/O invariant violated: read and write operations happened simultaneously");
        }

        if self.motor {
            self.cpu_delta += cycles;

            if (self.state == State::Playback || self.state == State::RecModeUncertain) && !self.have_read_out_1 {
                self.read_out_1_delta += cycles;
            }
        }

        if io.read_flag {

            if self.motor && (self.transitions_out <= 1) {
                if self.state == State::RecModeUncertain {
                    self.state = State::Playback;
                    info!("Started cassette playback.");
                }
                assert!(self.state == State::Playback);
            }

            // Heuristic to detect reading with Level 1 routines.
            //
            // If the routine paused too long after resetting the in_latch
            // before reading it again, assume it must be Level 1 code.
            if self.have_read_out_1 && self.read_out_1_delta > 0 {

                if (self.read_out_1_delta as f32) / CPU_MHZ > DETECT_250 {
                    self.cas_speed = Speed::S250;
                } else {
                    self.cas_speed = Speed::S500;
                }

                // Disable the detector.
                self.read_out_1_delta = 0;
            }

            io.read_flag = false;
        }

        else if io.write_flag {

            let latch_value = io.out_latch as i8;
            self.update_motor(io.motor_req, event_sink);

            if self.motor {
                if self.state == State::RecModeUncertain && latch_value != self.latch_lvl {
                    self.io_buffer_iter = self.iter_backup;
                    self.state = State::Recording;
                    event_sink.push(CassetteEvent::RecordingStarted);
                    info!("Started cassette recording.");
                }
                match self.state {
                    State::Playback => {
                        if !self.have_read_out_1 {
                            self.have_read_out_1 = true;
                        }
                    },
                    State::Recording => {
                        self.transition_out(OutVal::Level(latch_value), self.cpu_delta);
                        self.cpu_delta = 0;
                    },
                    _default => { },
                }
            }

            io.write_flag = false;
        }

        if self.motor && (self.state == State::Playback || self.state == State::RecModeUncertain) {

            while self.cpu_delta >= self.in_trans_delta {

                // Simulate analog signal processing on the cassette input:
                if (self.next_in_lvl != 0) && (self.latch_lvl == 0) {
                    io.in_latch = true;
                }

                // Deliver the previously read transition from the file:
                self.latch_lvl = self.next_in_lvl;
                self.cpu_delta -= self.in_trans_delta;

                // Read the next transition:
                self.transition_in();
            }
        }
        // TODO: Implement sound output support here.
        //
        //else {
        //    assert!(self.state == State::AudioOut);
        //    self.outputSoundSamples();
        //}
    }
    pub fn set_cassette_file<P: Into<path::PathBuf>>(&mut self, cassette_path: Option<P>) -> bool {
        if self.motor {
            error!("Cassette drive motor currently running, refusing to change the cassette file.");
            false
        } else {
            let (buffer, path, success) = match cassette_path {
                Some(path_in) => {
                    let path = path_in.into();

                    if path.exists() {
                        match fs::File::open(&path) {
                            Ok(mut file) => {
                                let mut buffer = Vec::new();
                                match file.read_to_end(&mut buffer) {
                                    Ok(_) => {
                                        info!("The cassette file `{}' was loaded into memory.", path.display());

                                        (Some(buffer), Some(path), true)
                                    },
                                    Err(error) => {
                                        error!("Failed to load `{}' into memory: {}.", path.display(), error);

                                        (None, None, false)
                                    },
                                }
                            },
                            Err(error) => {
                                error!("Couldn't open `{}' for reading: {}.", path.display(), error);

                                (None, None, false)
                            },
                        }
                    } else {
                        info!("Couldn't find `{}', creating...", path.display());
                        match fs::File::create(&path) {
                            Ok(..) => {
                                info!("Successfully created `{}'.", path.display());

                                (Some(Vec::new()), Some(path), true)
                            }
                            Err(error) => {
                                error!("Failed to create `{}': {}.", path.display(), error);

                                (None, None, false)
                            }
                        }
                    }
                },
                None => {
                    (None, None, true)
                },
            };

            self.io_buffer = buffer;
            self.cas_path  = path;
            success
        }
    }
    pub fn set_cassette_data_format(&mut self, format: Format) -> bool {
        if self.motor {
            error!("Cassette drive motor currently running, refusing to change the cassette file data format.");
            false
        } else {
            self.data_format = format;
            true
        }
    }
    pub fn set_cassette_file_offset(&mut self, offset: usize) -> bool {
        if self.motor {
            error!("Cassette drive motor currently running, refusing to seek the cassette.");
            false
        } else {
            self.io_buffer_iter = offset;
            true
        }
    }
    pub fn erase_cassette(&mut self) -> bool {
        if self.motor {
            error!("Cassette drive motor currently running, refusing to erase the cassette.");
            false
        } else {
            match self.io_buffer {
                Some(ref mut buffer) => {
                    buffer.clear();
                    self.io_buffer_iter = 0;

                    let path = match self.cas_path {
                        Some(ref path) => { path.clone() },
                        None => {
                            panic!("If a cassette is loaded, its path needs to be available; path missing");
                        },
                    };
                    match fs::File::create(&path) {
                        Ok(..) => {
                            info!("Cassette data erased.");
                            true
                        }
                        Err(error) => {
                            error!("Failed to overwrite `{}' with an empty file: {}.", path.display(), error);
                            false
                        }
                    }
                },
                None => {
                    warn!("No cassette in tape drive, nothing to erase.");
                    false
                },
            }
        }
    }
    pub fn power_off<ES: Sink<CassetteEvent>>(&mut self, io: &mut CassetteIO, event_sink: &mut ES) {

        self.update_motor(false, event_sink);

        self.avg              = 0.0;
        self.env              = 0.0;
        self.noise_floor      = 0;
        self.sample_rate      = 0;

        self.cpu_delta        = 0;
        self.have_read_out_1  = false;
        self.read_out_1_delta = 0;
        self.latch_lvl        = 0;
        self.next_in_lvl      = 0;
        self.transitions_out  = 0;
        self.in_trans_delta   = 0;
        self.roundoff_error   = 0.0;

        self.cas_pulse_state  = 0;
        self.cas_speed        = Speed::S500;
        self.cas_byte         = 0;
        self.cas_bit_num      = 0;

        io.read_flag          = false;
        io.write_flag         = false;
    }
    fn update_motor<ES: Sink<CassetteEvent>>(&mut self, new_motor_state: bool, event_sink: &mut ES) {
        if self.motor ^ new_motor_state {
            match new_motor_state {
                true => {
                    // Turning on the motor:
                    self.motor = true;
                    self.cpu_delta = 0;
                    self.latch_lvl = 0;
                    self.next_in_lvl = 0;
                    self.in_trans_delta = 0;
                    self.roundoff_error = 0.0;

                    self.cas_byte = 0;
                    self.cas_bit_num = 0;
                    self.cas_pulse_state = 0;
                    self.cas_speed = Speed::S500;

                    self.avg = NOISE_FLOOR as f32;
                    self.env = 127.0;

                    self.noise_floor = NOISE_FLOOR;
                    self.have_read_out_1 = false;
                    self.read_out_1_delta = 0;
                    self.transitions_out = 0;
                    self.iter_backup = self.io_buffer_iter;
                    self.state = State::RecModeUncertain;
                    event_sink.push(CassetteEvent::MotorStarted(self.io_buffer_iter));

                    info!("The cassette drive's motor was started.");
                },
                false => {
                    // Turning off the motor:
                    if self.state == State::Recording {
                        self.recording_stop_cleanup();
                    }
                    self.motor = false;
                    self.state = State::AudioOut;
                    event_sink.push(CassetteEvent::MotorStopped(self.io_buffer_iter));

                    info!("The cassette drive's motor was stopped.");
                },
            }
        }
    }
    fn recording_stop_cleanup(&mut self) {

        self.transition_out(OutVal::Flush, self.cpu_delta);

        match self.io_buffer {
            Some(ref buffer) => {
                match self.cas_path.clone() {
                    Some(path) => {
                        match fs::File::create(&path) {
                            Ok(mut file) => {
                                match file.write_all(&buffer) {
                                    Ok(()) => {
                                        info!("Saved the recorded cassette into `{}'.", path.display())
                                    },
                                    Err(error) => {
                                        error!("Failed to write the newly created tape into `{}': {}.", path.display(), error)
                                    },
                                }
                            },
                            Err(error) => {
                                error!("Failed to write the newly created tape into `{}': Couldn't open `{}' for writing: {}.", path.display(), path.display(), error)
                            },
                        }
                    },
                    None => {
                        panic!("The path for the loaded cassette is missing");
                    },
                }
            },
            None => {
                warn!("Your recording wasn't saved, since there's no cassette in the tape recorder.");
            },
        }
    }
    fn record_byte(&mut self, to_write: u8) {
        match self.io_buffer {
            Some(ref mut buffer) => {
                if self.io_buffer_iter >= buffer.len() {
                    buffer.resize(self.io_buffer_iter + 1, 0);
                }
                buffer[self.io_buffer_iter] = to_write;
            },
            None => { },
        };

        self.io_buffer_iter += 1;
    }
    fn retrieve_byte(&mut self) -> u8 {
        let retval = match self.io_buffer {
            Some(ref mut buffer) => {
                if self.io_buffer_iter < buffer.len() {
                    buffer[self.io_buffer_iter]
                } else {
                    0
                }
            },
            None => { 0 },
        };

        self.io_buffer_iter += 1;
        retval
    }

    // Record an output transition.
    //
    // out_lvl is the pulse state number (corresponding to a voltage level),
    // and flush is used to generate the final cassette file records even if
    // no transition had occured or if the final byte is incomplete.
    //
    // delta is the number of clock cycles since the last transition.
    //
    fn transition_out(&mut self, out_val: OutVal, delta: u32) {

        let (out_lvl, flush) = match out_val {
            OutVal::Level(lvl) => { (lvl, false) },
            OutVal::Flush      => { (self.latch_lvl, true) },
        };

        self.transitions_out += 1;
        if !flush && (out_lvl == self.latch_lvl) {
            return;
        }
        let ddelta_us: f32 = (delta as f32) / CPU_MHZ - self.roundoff_error;

        match self.data_format {
            Format::CAS => {
                if flush && (self.cas_bit_num != 0) {
                    let to_rec = self.cas_byte as u8;
                    self.record_byte(to_rec);
                    self.cas_byte = 0;
                    self.cas_bit_num = 0;
                }
                let mut sample: u8 = 2; // i.e., no bit.
                match self.cas_pulse_state {
                    ST_INITIAL => {
                        if (self.latch_lvl == 2) && (out_lvl == 0) {
                            // Low speed, end of first pulse.  Assume clock.
                            self.cas_pulse_state = ST_500GOTCLK;
                        }
                    },
                    ST_500GOTCLK => {
                        if (self.latch_lvl == 0) && (out_lvl == 1) {
                            // Low speed, start of next pulse.
                            if ddelta_us > ST_250THRESH {
                                // Oops, really ultra-low speed
                                // It's the next clock; bit was 0.
                                sample = 0;
                                // Watch for end of this clock.
                                self.cas_pulse_state = ST_250;
                            } else if ddelta_us > ST_500THRESH {
                                // It's the next clock; bit was 0.
                                sample = 0;
                                // Watch for end of this clock.
                                self.cas_pulse_state = ST_INITIAL;
                            } else {
                                // It's a data pulse; bit was 1.
                                sample = 1;
                                // Ignore the data pulse falling edge.
                                self.cas_pulse_state = ST_500GOTDAT;
                            }
                        }
                    },
                    ST_500GOTDAT => {
                        if (self.latch_lvl) == 2 && (out_lvl == 0) {
                            // End of data pulse; watch for end of next
                            // clock.
                            self.cas_pulse_state = ST_INITIAL;
                        }
                    },
                    ST_250 => {
                        if (self.latch_lvl == 2) && (out_lvl == 0) {
                            // Ultra-low speed, end of first pulse.
                            // Assume clock.
                            self.cas_pulse_state = ST_250GOTCLK;
                        }
                    },
                    ST_250GOTCLK => {
                        if (self.latch_lvl == 0) && (out_lvl == 1) {
                            if ddelta_us > ST_250THRESH {
                                // It's the next clock; bit was 0.
                                sample = 0;
                                // Watch for end of this clock.
                                self.cas_pulse_state = ST_250;
                            } else {
                                // It's a data pulse; bit was 1.
                                sample = 1;
                                // Ignore the data pulse falling edge.
                                self.cas_pulse_state = ST_250GOTDAT;
                            }
                        }
                    },
                    ST_250GOTDAT => {
                        if (self.latch_lvl == 2) && (out_lvl == 0) {
                            // End of data pulse; watch for end of next
                            // clock.
                            self.cas_pulse_state = ST_250;
                        }
                    },
                    _ => { () },
                }
                if sample != 2 {
                    self.cas_bit_num -= 1;
                    if self.cas_bit_num < 0 {
                        self.cas_bit_num = 7
                    }
                    self.cas_byte |= (sample << self.cas_bit_num) as i32;
                    if self.cas_bit_num == 0 {
                        let to_rec = self.cas_byte as u8;
                        self.record_byte(to_rec);
                        self.cas_byte = 0;
                    }
                }
            },
            Format::CPT => {
                let delta_us = (ddelta_us + 0.5) as u32;
                self.roundoff_error = ((delta_us as f32) - ddelta_us) as f32;

                if delta_us < 0x3FFF {
                    // Encode value and delta_us in two bytes if delta_us is
                    // small enough.
                    //
                    // Pack bits as ddddddddddddddvv and store this value in
                    // little-endian order.

                    let code = (out_lvl as u32) | (delta_us << 2);
                    self.record_byte(((code >> 0) & 0xFF) as u8);
                    self.record_byte(((code >> 8) & 0xFF) as u8);

                } else {
                    // Otherwise write a 0xffff escape code and encode
                    // in five bytes:
                    //
                    // 1-byte value, then 4-byte delta_us in little-endian
                    // order.

                    self.record_byte(0xFF);
                    self.record_byte(0xFF);
                    self.record_byte(out_lvl as u8);
                    self.record_byte(((delta_us >>  0) & 0xFF) as u8);
                    self.record_byte(((delta_us >>  8) & 0xFF) as u8);
                    self.record_byte(((delta_us >> 16) & 0xFF) as u8);
                    self.record_byte(((delta_us >> 24) & 0xFF) as u8);
                }
            },
        };

        self.latch_lvl = out_lvl;
    }
    // Read a new transition, updating self.next_in_lvl and self.in_trans_delta.
    fn transition_in(&mut self) {

        match self.data_format {
            Format::CAS => {
                if self.cas_pulse_state == 0 {
                    self.cas_bit_num -= 1;
                }
                if self.cas_bit_num < 0 {
                    self.cas_byte = self.retrieve_byte() as i32;
                    self.cas_bit_num = 7;
                }
                let current_bit = (self.cas_byte >> self.cas_bit_num) & 1;


                let mut last_state: bool = false;
                let mut delta_us:   i32;

                if current_bit == 0 {
                    match self.cas_speed {
                        Speed::S500 => {
                            delta_us  = S500_SHAPE_ZERO[self.cas_pulse_state].delta_us;
                            self.next_in_lvl = S500_SHAPE_ZERO[self.cas_pulse_state].next_lvl;
                            if S500_SHAPE_ZERO[self.cas_pulse_state + 1].next_lvl == -1 {
                                last_state = true;
                            }
                            if (self.cas_pulse_state == 3) && (self.cas_bit_num == 0) {
                                delta_us += 114;
                            }
                        },
                        Speed::S250 => {
                            delta_us  = S250_SHAPE_ZERO[self.cas_pulse_state].delta_us;
                            self.next_in_lvl = S250_SHAPE_ZERO[self.cas_pulse_state].next_lvl;
                            if S250_SHAPE_ZERO[self.cas_pulse_state + 1].next_lvl == -1 {
                                last_state = true;
                            }
                            if (self.cas_pulse_state == 6) && (self.cas_bit_num == 0) {
                                delta_us += 112;
                            }
                        },
                    };
                } else {
                    match self.cas_speed {
                        Speed::S500 => {
                            delta_us  = S500_SHAPE_ONE[self.cas_pulse_state].delta_us;
                            self.next_in_lvl = S500_SHAPE_ONE[self.cas_pulse_state].next_lvl;
                            if S500_SHAPE_ONE[self.cas_pulse_state + 1].next_lvl == -1 {
                                last_state = true;
                            }
                        },
                        Speed::S250 => {
                            delta_us  = S250_SHAPE_ONE[self.cas_pulse_state].delta_us;
                            self.next_in_lvl = S250_SHAPE_ONE[self.cas_pulse_state].next_lvl;
                            if S250_SHAPE_ONE[self.cas_pulse_state + 1].next_lvl == -1 {
                                last_state = true;
                            }
                        },
                    }
                }
                if !last_state {
                    self.cas_pulse_state += 1;
                } else {
                    self.cas_pulse_state = 0;

                    // Kludge to emulate extra delay that's needed after the
                    // initial 0xA5 sync byte to let Basic execute the
                    // CLEAR routine:

                    if (self.cas_byte == 0xA5) && (self.cas_speed == Speed::S500) {
                        delta_us += 1034;
                    }
                }
                let delta_ts = (delta_us as f32) * CPU_MHZ - self.roundoff_error;
                self.in_trans_delta = (delta_ts + 0.5) as u32;
                self.roundoff_error = (self.in_trans_delta as f32) - delta_ts;
            },
            Format::CPT => {
                let low: u8 = self.retrieve_byte();
                let high: u8 = self.retrieve_byte();

                let code: u16 = ((high as u16) << 8) | (low as u16);
                let delta_us: u32;
                if code == 0xFFFF {
                    self.next_in_lvl = self.retrieve_byte() as i8;
                    let d_lsb: u8 = self.retrieve_byte();
                    let d_3rd: u8 = self.retrieve_byte();
                    let d_2nd: u8 = self.retrieve_byte();
                    let d_msb: u8 = self.retrieve_byte();

                    delta_us = ((d_msb as u32) << 24)
                             | ((d_2nd as u32) << 16)
                             | ((d_3rd as u32) <<  8)
                             | ((d_lsb as u32) <<  0);
                } else {
                    self.next_in_lvl = (code & 3) as i8;
                    delta_us = (code >> 2) as u32;
                }
                let delta_ts: f32 = (delta_us as f32) * CPU_MHZ - self.roundoff_error;
                self.in_trans_delta = (delta_ts + 0.5) as u32;
                self.roundoff_error = (self.in_trans_delta as f32) - delta_ts;
            },
        }
    }
}
