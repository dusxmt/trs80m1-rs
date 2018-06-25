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
   Modified by Marek Benc, 2017, 2018, to adapt it for the rust-based
   trs80m1-rs emulator.
 */

use std::path;
use std::fs;
use std::io::Read;
use std::io::Write;

use proj_config;
use memory;
use emulator;
use util;
use util::MessageLogging;


const CPU_MHZ:    f32 = (emulator::CPU_HZ as f32) / (1_000_000 as f32);
const DETECT_250: f32 = 1200.0;   // For level 1 input routine detection.


#[derive(Copy, Clone, PartialEq, Debug)] // For the config system.
pub enum Format {
    CAS,  // Recovered bit/byte stream.
    CPT,  // Cassette pulse train w/ exact timing.
}

#[derive(Copy, Clone, PartialEq)]
enum State {
    Close,
    Read,
    Write,
    Failed,
}

#[derive(PartialEq)]
enum Speed {
    S500,
    S250,
}

#[derive(PartialEq)]
enum OutVal {
    Value(i32),
    Flush,
}

const NOISE_FLOOR: i32 = 64;

// Pulse shapes for conversion from .cas on input:
struct PulseShape {
    delta_us: i32,
    next:     i32,
}

static S500_SHAPE_ZERO: [PulseShape; 5] =
  [ PulseShape { delta_us: 0,    next: 1  },
    PulseShape { delta_us: 128,  next: 2  },
    PulseShape { delta_us: 128,  next: 0  },
    PulseShape { delta_us: 1757, next: 0  },    /* 1871 after 8th bit */
    PulseShape { delta_us: -1,   next: -1 } ];

static S500_SHAPE_ONE: [PulseShape; 8] =
  [ PulseShape { delta_us: 0,    next: 1  },
    PulseShape { delta_us: 128,  next: 2  },
    PulseShape { delta_us: 128,  next: 0  },
    PulseShape { delta_us: 748,  next: 1  },
    PulseShape { delta_us: 128,  next: 2  },
    PulseShape { delta_us: 128,  next: 0  },
    PulseShape { delta_us: 748,  next: 0  },    /* 860 after 8th bit; 1894 after a5 sync */
    PulseShape { delta_us: -1,   next: -1 } ];

static S250_SHAPE_ZERO: [PulseShape; 5] =
  [ PulseShape { delta_us: 0,    next: 1  },
    PulseShape { delta_us: 125,  next: 2  },
    PulseShape { delta_us: 125,  next: 0  },
    PulseShape { delta_us: 3568, next: 0  },
    PulseShape { delta_us: -1,   next: -1 } ];

static S250_SHAPE_ONE: [PulseShape; 8] =
  [ PulseShape { delta_us: 0,    next: 1  },
    PulseShape { delta_us: 128,  next: 2  },
    PulseShape { delta_us: 128,  next: 0  },
    PulseShape { delta_us: 1673, next: 1  },
    PulseShape { delta_us: 128,  next: 2  },
    PulseShape { delta_us: 128,  next: 0  },
    PulseShape { delta_us: 1673, next: 0  },
    PulseShape { delta_us: -1,   next: -1 } ];

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


pub struct CassetteRecorder {
    in_cas_path:     Option<path::PathBuf>,
    out_cas_path:    Option<path::PathBuf>,
    in_format:       Format,
    out_format:      Format,
    state:           State,
    motor:           bool,
    io_buffer:       Option<Vec<u8>>,
    io_buffer_iter:  usize,
    avg:             f32,
    env:             f32,
    noise_floor:     i32,
    sample_rate:     i32,

    // For bit-level emulation:
    transition:      u32,
    last_sound:      u32,
    first_out_read:  u32,
    value:           i32,
    next:            i32,
    flipflop:        bool,
    transitions_out: i32,
    delta:           u32,
    roundoff_error:  f32,

    // For bit/byte conversion (.cas file i/o):
    cas_pulse_state: usize,
    cas_speed:       Speed,
    cas_byte:        i32,
    cas_bit_num:     i32,

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl memory::PeripheralIO for CassetteRecorder {
    fn peripheral_read_byte(&mut self, addr: u16, cycle_timestamp: u32) -> u8 {
        let port: u8 = (addr & 0x00FF) as u8;

        if port != 0xFF {
            panic!("Failed read: Port 0x{:02X} is invalid for the cassette recorder", port);
        }

        if self.motor && (self.transitions_out <= 1) {
            self.assert_state(State::Read);
        }

        // Heuristic to detect reading with Level 1 routines.
        //
        // If the routine paused too long after resetting the flipflop
        // before reading it again, assume it must be Level 1 code.
        if self.first_out_read > 1 {
            if (cycle_timestamp.wrapping_sub(self.first_out_read) as f32) / CPU_MHZ > DETECT_250 {
                self.cas_speed = Speed::S250;
            } else {
                self.cas_speed = Speed::S500;
            }

            // Disable the detector.
            self.first_out_read = 1;
        }
        self.update(cycle_timestamp);

        if self.flipflop { 0b1111_1111 } else { 0b0111_1111 }
    }
    fn peripheral_write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32) {
        let port: u8 = (addr & 0x00FF) as u8;

        if (addr & 0x00FF) != 0xFF {
            panic!("Failed write: Port 0x{:02X} is invalid for the cassette recorder", port);
        }

        self.update_motor((val & 0b0000_0100) != 0, cycle_timestamp);
        let out_val = (val & 0x03) as i32;

        if self.motor {
            if self.state == State::Read {
                self.update(cycle_timestamp);
                self.flipflop = false;
                if self.first_out_read == 0 {
                    self.first_out_read = cycle_timestamp;
                }
            }
            if (self.state != State::Read) && (out_val != self.value) {
                if self.assert_state(State::Write) >= 0 {
                    self.transition_out(OutVal::Value(out_val), cycle_timestamp);
                }
            }
        }
    }
}

impl CassetteRecorder {
    pub fn new(config_system: &proj_config::ConfigSystem, _startup_logger: &mut util::StartupLogger) -> Option<CassetteRecorder> {
        let mut recorder = CassetteRecorder {
                               in_cas_path:     match config_system.config_items.cassette_input_cassette {
                                                    Some(ref entry) => {
                                                        let mut path = config_system.config_dir_path.clone();
                                                        path.push(entry);
                                                        Some(path)
                                                    },
                                                    None => { None },
                                                },
                               out_cas_path:    match config_system.config_items.cassette_output_cassette {
                                                    Some(ref entry) => {
                                                        let mut path = config_system.config_dir_path.clone();
                                                        path.push(entry);
                                                        Some(path)
                                                    },
                                                    None => { None },
                                                },
                               in_format:       config_system.config_items.cassette_input_cassette_format,
                               out_format:      config_system.config_items.cassette_output_cassette_format,
                               state:           State::Close,
                               motor:           false,
                               io_buffer:       None,
                               io_buffer_iter:  0,
                               avg:             0.0,
                               env:             0.0,
                               noise_floor:     0,
                               sample_rate:     0,

                               // For bit-level emulation:
                               transition:      0,
                               last_sound:      0,
                               first_out_read:  0,
                               value:           0,
                               next:            0,
                               flipflop:        false,
                               transitions_out: 0,
                               delta:           0,
                               roundoff_error:  0.0,

                               // For bit/byte conversion (.cas file i/o):
                               cas_pulse_state: 0,
                               cas_speed:       Speed::S500,
                               cas_byte:        0,
                               cas_bit_num:     0,

                               logged_messages:  Vec::new(),
                               messages_present: false,
                           };

        recorder.log_message("Created the cassette recorder.".to_owned());
        Some(recorder)
    }
    pub fn update_cassette_paths(&mut self, config_system: &proj_config::ConfigSystem) {
        self.in_cas_path  = match config_system.config_items.cassette_input_cassette {
                                Some(ref entry) => {
                                    let mut path = config_system.config_dir_path.clone();
                                    path.push(entry);
                                    Some(path)
                                },
                                None => { None },
                            };
        self.out_cas_path = match config_system.config_items.cassette_output_cassette {
                                Some(ref entry) => {
                                    let mut path = config_system.config_dir_path.clone();
                                    path.push(entry);
                                    Some(path)
                                },
                                None => { None },
                            };
    }
    pub fn update_cassette_formats(&mut self, config_system: &proj_config::ConfigSystem) {
        self.in_format  = config_system.config_items.cassette_input_cassette_format;
        self.out_format = config_system.config_items.cassette_output_cassette_format;
    }
    pub fn power_off(&mut self) {
        let old_state = self.state;
        let old_motor = self.motor;

        self.state            = State::Close;
        self.motor            = false;
        self.io_buffer        = None;
        self.io_buffer_iter   = 0;

        self.avg              = 0.0;
        self.env              = 0.0;
        self.noise_floor      = 0;
        self.sample_rate      = 0;

        self.transition       = 0;
        self.last_sound       = 0;
        self.first_out_read   = 0;
        self.value            = 0;
        self.next             = 0;
        self.flipflop         = false;
        self.transitions_out  = 0;
        self.delta            = 0;
        self.roundoff_error   = 0.0;

        self.cas_pulse_state  = 0;
        self.cas_speed        = Speed::S500;
        self.cas_byte         = 0;
        self.cas_bit_num      = 0;

        if old_motor {
            self.log_message("The cassette drive's motor was stopped.".to_owned());
        }
        if old_state != State::Close {
            self.log_message("The cassette drive was turned off.".to_owned());
        }
    }
    fn update_motor(&mut self, new_state: bool, cycle_timestamp: u32) {
        if self.motor ^ new_state {
            match new_state {
                true => {
                    // Turning on the motor:
                    self.motor = true;
                    self.transition = cycle_timestamp;
                    self.value = 0;
                    self.next = 0;
                    self.delta = 0;
                    self.flipflop = false;
                    self.roundoff_error = 0.0;

                    self.cas_byte = 0;
                    self.cas_bit_num = 0;
                    self.cas_pulse_state = 0;
                    self.cas_speed = Speed::S500;

                    self.avg = NOISE_FLOOR as f32;
                    self.env = 127.0;

                    self.noise_floor = NOISE_FLOOR;
                    self.first_out_read = 0;
                    self.transitions_out = 0;
                    self.log_message("The cassette drive's motor was started.".to_owned());
                },
                false => {
                    // Turning off the motor:
                    if self.state == State::Write {
                        self.transition_out(OutVal::Flush, cycle_timestamp);
                    }
                    self.assert_state(State::Close);
                    self.motor = false;
                    self.log_message("The cassette drive's motor was stopped.".to_owned());
                },
            }
        }
    }
    // Return value: 1 = already that state; 0 = state changed; -1 = failed.
    fn assert_state(&mut self, new_state: State) -> i32 {
        if self.state == new_state {
           1
        } else if (self.state == State::Failed) && (new_state != State::Close) {
           -1
        } else {
            match new_state {
                State::Read => {
                    match self.in_cas_path.clone() {
                        Some(ref path) => {
                            let mut file = match fs::File::open(path) {
                                Ok(file) => { file },
                                Err(error) => {
                                    self.log_message(format!("Couldn't open `{}\' for reading: {}.", path.display(), error));
                                    self.state = State::Failed;
                                    return -1;
                                },
                            };
                            let mut buffer = Vec::new();
                            match file.read_to_end(&mut buffer) {
                                Ok(_) => { () },
                                Err(error) => {
                                    self.log_message(format!("Failed to load `{}\' into memory: {}.", path.display(), error));
                                    self.state = State::Failed;
                                    return -1;
                                },
                            };
                            self.io_buffer = Some(buffer);
                            self.io_buffer_iter = 0;
                            let message = format!("The cassette file `{}\' was loaded into memory.", path.display());
                            self.log_message(message);
                        },
                        None => {
                            self.log_message("Warning: no input cassette specified in the config file.".to_owned());
                            self.io_buffer = Some(Vec::new());
                            self.io_buffer_iter = 0;
                        },
                    };
                },
                State::Write => {
                    self.io_buffer = Some(Vec::new());
                },
                State::Close => {
                    if self.state == State::Write {
                        let buffer = match self.io_buffer {
                            Some(ref buffer) => { buffer.clone() },
                            None => {
                                panic!("No io_buffer present in the cassette mechanism after finishing a write");
                            },
                        };
                        match self.out_cas_path.clone() {
                            Some(ref path) => {
                                match fs::File::create(path) {
                                    Ok(mut file) => {
                                        match file.write_all(&buffer) {
                                            Ok(()) => {
                                                self.log_message(format!("Saved the recorded cassette into `{}\'.",
                                                    path.display())); 
                                            },
                                            Err(error) => {
                                                self.log_message(format!("Failed to write the newly created tape into `{}\': {}.", path.display(), error));
                                            },
                                        };
                                    },
                                    Err(error) => {
                                        self.log_message(format!("Failed to write the newly created tape into `{}\': Couldn't open `{}\' for writing: {}.", path.display(), path.display(), error));
                                    },
                                };
                            },
                            None => {
                                self.log_message("Warning: no output cassette specified in the config file, discarding the recorded data.".to_owned());
                            },
                        };
                    }
                    self.io_buffer = None;
                },
                State::Failed => {
                    self.io_buffer = None;
                },
            }
            self.state = new_state;
            0
        }
    }
    // Record an output transition.
    //
    // The value is either the new port Value(value), or Flush
    fn transition_out(&mut self, value_in: OutVal, cycle_timestamp: u32) {
        use self::OutVal::*;

        self.transitions_out += 1;
        if value_in == Value(self.value) {
            return;
        }
        let ddelta_us: f32 = (cycle_timestamp.wrapping_sub(self.transition) as f32) / CPU_MHZ - self.roundoff_error;

        match self.out_format {
            Format::CAS => {
                if (value_in == Flush) && (self.cas_bit_num != 0) {
                    let buffer = match self.io_buffer {
                        Some(ref mut buffer) => { buffer },
                        None => {
                            panic!("No io_buffer present in the cassette mechanism during a write");
                        },
                    };
                    buffer.push(self.cas_byte as u8);
                    self.cas_byte = 0;
                } else {
                    let mut sample: u8 = 2; // i.e., no bit.
                    match self.cas_pulse_state {
                        ST_INITIAL => {
                            if (self.value == 2) && (value_in == Value(0)) {
                                // Low speed, end of first pulse.  Assume clock.
                                self.cas_pulse_state = ST_500GOTCLK;
                            }
                        },
                        ST_500GOTCLK => {
                            if (self.value == 0) && (value_in == Value(1)) {
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
                            if (self.value) == 2 && (value_in == Value(0)) {
                                // End of data pulse; watch for end of next
                                // clock.
                                self.cas_pulse_state = ST_INITIAL;
                            }
                        },
                        ST_250 => {
                            if (self.value == 2) && (value_in == Value(0)) {
                                // Ultra-low speed, end of first pulse.
                                // Assume clock.
                                self.cas_pulse_state = ST_250GOTCLK;
                            }
                        },
                        ST_250GOTCLK => {
                            if (self.value == 0) && (value_in == Value(1)) {
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
                            if (self.value == 2) && (value_in == Value(0)) {
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
                            let buffer = match self.io_buffer {
                                Some(ref mut buffer) => { buffer },
                                None => {
                                    panic!("No io_buffer present in the cassette mechanism during a write");
                                },
                            };
                            buffer.push(self.cas_byte as u8);
                            self.cas_byte = 0;
                        }
                    }
                }
            },
            Format::CPT => {
                let buffer = match self.io_buffer {
                    Some(ref mut buffer) => { buffer },
                    None => {
                        panic!("No io_buffer present in the cassette mechanism during a write");
                    },
                };
                let value = match value_in {
                    Value(value) => { value as u8 },
                    Flush => { self.value as u8 },
                };
                let delta_us = (ddelta_us + 0.5) as u32;
                self.roundoff_error = ((delta_us as f32) - ddelta_us) as f32;

                if delta_us < 0x3FFF {
                    // Encode value and delta_us in two bytes if delta_us is
                    // small enough.
                    //
                    // Pack bits as ddddddddddddddvv and store this value in
                    // little-endian order.

                    let code = (value as u32) | (delta_us << 2);
                    buffer.push(((code >> 0) & 0xFF) as u8);
                    buffer.push(((code >> 8) & 0xFF) as u8);

                } else {
                    // Otherwise write a 0xffff escape code and encode
                    // in five bytes:
                    //
                    // 1-byte value, then 4-byte delta_us in little-endian
                    // order.

                    buffer.push(0xFF);
                    buffer.push(0xFF);
                    buffer.push(value);
                    buffer.push(((delta_us >>  0) & 0xFF) as u8);
                    buffer.push(((delta_us >>  8) & 0xFF) as u8);
                    buffer.push(((delta_us >> 16) & 0xFF) as u8);
                    buffer.push(((delta_us >> 24) & 0xFF) as u8);
                }
            },
        };

        self.transition = cycle_timestamp;
        self.value = match value_in {
            Value(value) => { value },
            Flush => { -500 },
        }
    }
    // Read a new transition, updating self.next and self.delta.
    // If input fails (perhaps due to eof), return 0, otherwise 1.
    //
    // Set self.delta to 0xFFFFFFFF on failure.
    //
    fn transition_in(&mut self) -> i32 {
        let ret: i32;

        match self.in_format {
            Format::CAS => {
                if self.cas_pulse_state == 0 {
                    self.cas_bit_num -= 1;
                }
                if self.cas_bit_num < 0 {
                    let buffer = match self.io_buffer {
                        Some(ref buffer) => { buffer },
                        None => {
                            panic!("No io_buffer present in the cassette mechanism during a read");
                        },
                    };
                    let in_byte: i32;
                    if self.io_buffer_iter < buffer.len() {
                        in_byte = buffer[self.io_buffer_iter] as i32;
                        self.io_buffer_iter += 1;
                    } else {
                        // Add one extra zero byte to work around an apparent
                        // bug in the Vavasour Model I emulator's .CAS files.
                        if self.cas_byte != 0x100 {
                            in_byte = 0x100;
                        } else {
                            self.delta = 0xFFFFFFFF;
                            return 0;
                        }
                    }
                    self.cas_byte = in_byte;
                    self.cas_bit_num = 7;
                }
                let current_bit = (self.cas_byte >> self.cas_bit_num) & 1;


                let mut last_state: bool = false;
                let mut delta_us:   i32;

                if current_bit == 0 {
                    match self.cas_speed {
                        Speed::S500 => {
                            delta_us  = S500_SHAPE_ZERO[self.cas_pulse_state].delta_us;
                            self.next = S500_SHAPE_ZERO[self.cas_pulse_state].next;
                            if S500_SHAPE_ZERO[self.cas_pulse_state + 1].next == -1 {
                                last_state = true;
                            }
                            if (self.cas_pulse_state == 3) && (self.cas_bit_num == 0) {
                                delta_us += 114;
                            }
                        },
                        Speed::S250 => {
                            delta_us  = S250_SHAPE_ZERO[self.cas_pulse_state].delta_us;
                            self.next = S250_SHAPE_ZERO[self.cas_pulse_state].next;
                            if S250_SHAPE_ZERO[self.cas_pulse_state + 1].next == -1 {
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
                            self.next = S500_SHAPE_ONE[self.cas_pulse_state].next;
                            if S500_SHAPE_ONE[self.cas_pulse_state + 1].next == -1 {
                                last_state = true;
                            }
                        },
                        Speed::S250 => {
                            delta_us  = S250_SHAPE_ONE[self.cas_pulse_state].delta_us;
                            self.next = S250_SHAPE_ONE[self.cas_pulse_state].next;
                            if S250_SHAPE_ONE[self.cas_pulse_state + 1].next == -1 {
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
                self.delta = (delta_ts + 0.5) as u32;
                self.roundoff_error = (self.delta as f32) - delta_ts;

                ret = 1;
            },
            Format::CPT => {
                let buffer = match self.io_buffer {
                    Some(ref buffer) => { buffer },
                    None => {
                        panic!("No io_buffer present in the cassette mechanism during a read");
                    },
                };
                if (self.io_buffer_iter + 1) < buffer.len() {
                    let low: u8 = buffer[self.io_buffer_iter];
                    let high: u8 = buffer[self.io_buffer_iter + 1];
                    self.io_buffer_iter += 2;

                    let code: u16 = ((high as u16) << 8) | (low as u16);
                    let delta_us: u32;
                    if code == 0xFFFF {
                        if (self.io_buffer_iter + 4) < buffer.len() {
                            self.next = buffer[self.io_buffer_iter] as i32;
                            let d_lsb: u8 = buffer[self.io_buffer_iter + 1];
                            let d_3rd: u8 = buffer[self.io_buffer_iter + 2];
                            let d_2nd: u8 = buffer[self.io_buffer_iter + 3];
                            let d_msb: u8 = buffer[self.io_buffer_iter + 4];

                            self.io_buffer_iter += 5;

                            delta_us = ((d_msb as u32) << 24)
                                     | ((d_2nd as u32) << 16)
                                     | ((d_3rd as u32) <<  8)
                                     | ((d_lsb as u32) <<  0);
                        } else {
                            self.delta = 0xFFFFFFFF;
                            return 0;
                        }
                    } else {
                        self.next = (code & 3) as i32;
                        delta_us = (code >> 2) as u32;
                    }
                    let delta_ts: f32 = (delta_us as f32) * CPU_MHZ - self.roundoff_error;
                    self.delta = (delta_ts + 0.5) as u32;
                    self.roundoff_error = (self.delta as f32) - delta_ts;
                    ret = 1;
                } else {
                    self.delta = 0xFFFFFFFF;
                    return 0;
                }
            },
        }
        ret
    }
    fn update(&mut self, cycle_timestamp: u32) {
        if self.motor && (self.state != State::Write) &&
            (self.assert_state(State::Read) >= 0) {

            while cycle_timestamp.wrapping_sub(self.transition) >= self.delta {

                // Simulate analog signal processing on the cassette input:
                if (self.next != 0) && (self.value == 0) {
                    self.flipflop = true;
                }

                // Deliver the previously read transition from the file:
                self.value = self.next;
                self.transition = self.transition.wrapping_add(self.delta);

                // Read the next transition:
                self.transition_in();
            }
        }
    }
}

impl MessageLogging for CassetteRecorder {
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
