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

use std::io::prelude::*;
use std::fs;
use std::path;

use proj_config;
use keyboard;
use video;
use cassette;

use util;
use util::MessageLogging;


// Memory layout:

// 12K BASIC ROM chip:
pub const ROM_BASE: u16 = 0x0000;
pub const ROM_SIZE: u16 = 0x3000;

// RAM:
pub const RAM_BASE: u16 = 0x4000;

// Keyboard:
pub const KBD_BASE: u16 = 0x3800;
use keyboard::KBD_MEM_SIZE as KBD_SIZE;

// Video display:
pub const VID_BASE: u16 = 0x3C00;
use video::VID_MEM_SIZE as VID_SIZE;

// A memory device is one that implements the read and write operations.
pub trait MemIO {
    fn read_byte(&mut self, addr: u16, cycle_timestamp: u32) -> u8;
    fn write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32);

    fn read_word(&mut self, addr: u16, cycle_timestamp: u32) -> u16 {
        let lsb = self.read_byte(addr, cycle_timestamp);
        let msb = self.read_byte(addr + 1, cycle_timestamp);

        ((msb as u16) << 8) | (lsb as u16)
    }

    fn write_word(&mut self, addr: u16, val: u16, cycle_timestamp: u32) {
        self.write_byte(addr, (val & 0xff) as u8, cycle_timestamp);
        self.write_byte(addr + 1, ((val >> 8) & 0xff) as u8, cycle_timestamp);
    }
}

// A peripheral device is very similar, except it's accessed differently.
pub trait PeripheralIO {
    fn peripheral_read_byte(&mut self, addr: u16, cycle_timestamp: u32) -> u8;
    fn peripheral_write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32);

    fn peripheral_read_word(&mut self, addr: u16, cycle_timestamp: u32) -> u16 {
        let lsb = self.peripheral_read_byte(addr, cycle_timestamp);
        let msb = self.peripheral_read_byte(addr + 1, cycle_timestamp);

        ((msb as u16) << 8) | (lsb as u16)
    }

    fn peripheral_write_word(&mut self, addr: u16, val: u16, cycle_timestamp: u32) {
        self.peripheral_write_byte(addr, (val & 0xff) as u8, cycle_timestamp);
        self.peripheral_write_byte(addr + 1, ((val >> 8) & 0xff) as u8, cycle_timestamp);
    }
}

pub trait MemoryChip {
    fn chip_id(&self) -> &str;
    fn chip_data(&self) -> &[u8];
    fn chip_data_mut(&mut self) -> &mut [u8];
    fn default_value(&self) -> u8;
}

pub trait MemoryChipOps {
    fn wipe(&mut self);
    fn load_from_file<P: AsRef<path::Path>>(&mut self, path: P, offset: u16) -> bool;
    fn load_from_buffer(&mut self, input_buffer: &[u8], input_name: &str, offset: u16);
}

// A RAM memory chip:
pub struct RamChip {
    id:    String,
    data:  Box<[u8]>,

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl RamChip {
    // Create a new ram chip.
    pub fn new(id: String, size: u16, start_addr: u16) -> RamChip {

        let mut chip = RamChip {
                           id:   id,
                           data: vec![0; size as usize].into_boxed_slice(),

                           logged_messages:  Vec::new(),
                           messages_present: false,
                       };

        let create_report = if (size % 1024) == 0 {
                                format!("Created ram chip `{}', starting address: 0x{:04X}, size: {}K.", chip.id, start_addr, size / 1024)
                            } else {
                                format!("Created ram chip `{}', starting address: 0x{:04X}, size: {} bytes.", chip.id, start_addr, size)
                            };

        chip.log_message(create_report);
        chip
    }
}

impl MemIO for RamChip {
    fn read_byte(&mut self, addr: u16, _cycle_timestamp: u32) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for RAM chip `{}'", addr, self.id);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8, _cycle_timestamp: u32) {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize] = val;
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for RAM chip `{}'", addr, self.id);
        }
    }
}

// A ROM memory chip:
pub struct RomChip {
    id:    String,
    data:  Box<[u8]>,

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl RomChip {
    // Create a new rom chip.
    pub fn new(id: String, size: u16, start_addr: u16) -> RomChip {
        let mut chip = RomChip {
                           id:   id,
                           data: vec![0xFF; size as usize].into_boxed_slice(),

                           logged_messages:  Vec::new(),
                           messages_present: false,
                       };

        let create_report = if (size % 1024) == 0 {
                                format!("Created rom chip `{}', starting address: 0x{:04X}, size: {}K.", chip.id, start_addr, size / 1024)
                            } else {
                                format!("Created rom chip `{}', starting address: 0x{:04X}, size: {} bytes.", chip.id, start_addr, size)
                            };

        chip.log_message(create_report);
        chip
    }
}

impl MemIO for RomChip {
    fn read_byte(&mut self, addr: u16, _cycle_timestamp: u32) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for ROM chip `{}'", addr, self.id);
        }
    }
    fn write_byte(&mut self, addr: u16, _val: u8, _cycle_timestamp: u32) {
        if (addr as usize) < self.data.len() {
            let id = self.id.clone();
            self.log_message(format!("Warning: Failed write: Invalid operation for ROM chip `{}'.", id));
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for ROM chip `{}'", addr, self.id);
        }
    }
}

impl MessageLogging for RamChip {
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

impl MessageLogging for RomChip {
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

impl MemoryChip for RamChip {
    fn chip_id(&self) -> &str {
        &self.id
    }
    fn chip_data(&self) -> &[u8] {
        &self.data
    }
    fn chip_data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn default_value(&self) -> u8 {
        0x00
    }
}

impl MemoryChip for RomChip {
    fn chip_id(&self) -> &str {
        &self.id
    }
    fn chip_data(&self) -> &[u8] {
        &self.data
    }
    fn chip_data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn default_value(&self) -> u8 {
        0xFF
    }
}

impl<T: MemoryChip + MessageLogging> MemoryChipOps for T {

    // Erase the contents of a memory chip:
    fn wipe(&mut self) {

        let size = self.chip_data_mut().len();
        let default_value = self.default_value();

        let mut index = 0;

        while index < size {
            self.chip_data_mut()[index] = default_value;
            index += 1;
        }

        let id = self.chip_id().to_owned();
        self.log_message(format!("The memory chip `{}' was wiped.", id));
    }

    // Load a file into a memory chip:
    fn load_from_file<P: AsRef<path::Path>>(&mut self, path_in: P, offset: u16) -> bool {
        let id   = self.chip_id().to_owned();
        let path = path_in.as_ref() as &path::Path;

        if (offset as usize) >= self.chip_data_mut().len() {
            self.log_message(format!("Failed to load the file `{}' into `{}', offset 0x{:04X}: Offset out of range.", path.display(), id, offset));

            return false;
        }
        match fs::File::open(path) {
            Ok(mut file) => {

                let mut read_buffer = Vec::with_capacity(self.chip_data_mut().len());

                match file.read_to_end(&mut read_buffer) {
                    Ok(read_len) => {
                        let can_load_into_mem = self.chip_data_mut().len() - (offset as usize);
                        let load_count = if read_len > can_load_into_mem { can_load_into_mem } else { read_len };

                        let mut mem_index  = offset as usize;
                        let mut file_index = 0;
                        while file_index < load_count {
                            self.chip_data_mut()[mem_index] = read_buffer[file_index];

                            mem_index += 1;
                            file_index += 1;
                        }
                        self.log_message(format!("Loaded {} bytes from `{}' into `{}', offset 0x{:04X}.", load_count, path.display(), id, offset));
                        if read_len > load_count {
                            self.log_message(format!("Note: {} bytes from `{}' ({} bytes large) didn't fit.", read_len - load_count, path.display(), read_len));
                        }
                        true
                    },
                    Err(error) => {
                        self.log_message(format!("Failed to load the file `{}' into `{}', offset 0x{:04X}: File reading error: {}.", path.display(), id, offset, error));
                        false
                    },
                }
            },
            Err(error) => {
                self.log_message(format!("Failed to load the file `{}' into `{}', offset 0x{:04X}: File opening error: {}.", path.display(), id, offset, error));
                false
            }
        }
    }

    fn load_from_buffer(&mut self, input_buffer: &[u8], input_name: &str, offset: u16) {
        let id = self.chip_id().to_owned();
        let input_len = input_buffer.len();

        let can_load_into_mem = self.chip_data_mut().len() - (offset as usize);
        let load_count = if input_len > can_load_into_mem { can_load_into_mem } else { input_len };

        let mut mem_index  = offset as usize;
        let mut input_index = 0;
        while input_index < load_count {
            self.chip_data_mut()[mem_index] = input_buffer[input_index];

            mem_index += 1;
            input_index += 1;
        }
        self.log_message(format!("Loaded {} bytes from `{}' into `{}', offset 0x{:04X}.", load_count, input_name, id, offset));
        if input_len > load_count {
            self.log_message(format!("Note: {} bytes from `{}' ({} bytes large) didn't fit.", input_len - load_count, input_name, input_len));
        }
    }
}

// Write a hexdump of the given data buffer into the formatter `f`.
//fn hexdump(f: &mut fmt::Formatter, data: &[u8]) -> fmt::Result {
//    let mut row_iter:    usize;
//    let mut column_iter: usize;
//
//    row_iter = 0;
//    while row_iter < data.len() {
//        try!(write!(f, "    {:04X}:", row_iter));
//        column_iter = 0;
//        while (row_iter + column_iter) < data.len() && column_iter < 16 {
//            try!(write!(f, " {:02X}", data[row_iter + column_iter]));
//            column_iter += 0x1;
//        }
//        try!(write!(f, "\n"));
//        row_iter += 0x10;
//    }
//    Ok(())
//}
//
//impl<T: MemoryChip> fmt::Debug for T {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        let chip_id = self.chip_id().to_owned();
//        let data    = self.chip_data();
//        try!(write!(f, "MemoryChip {{\n    id: `{}',\n    Size of the chip: {} (0x{:04X}),\n\n    Hex dump:\n",
//            chip_id, data.len(), data.len()));
//        try!(hexdump(f, data));
//        try!(write!(f, "}}"));
//
//        Ok(())
//    }
//}


pub struct MemorySystem {
    pub ram_chip: RamChip,
    pub rom_chip: RomChip,
    pub kbd_mem:  keyboard::KeyboardMemory,
    pub vid_mem:  video::VideoMemory,

    pub cas_rec:  cassette::CassetteRecorder,

    // The interrupt request interface is a part of the memory system, to
    // allow any peripheral on the system bus to be able to issue an interrupt
    // request.  The following variables, when set to true, will request a
    // non-maskable or maskable interrupt respectively:
    pub nmi_request: bool,
    pub int_request: bool,

    // Because the most common use of the Mode 0 interrupts is for the
    // interrupting device to send a reset instruction (even though it
    // can send an instruction of arbitrary length), for the sake of
    // simplicity, this implementation makes only that a possibility:
    pub mode0_int_addr:   u16,
    pub mode2_int_vec:    u8,

    logged_messages:  Vec<String>,
    messages_present: bool,
}

impl MemorySystem {
    pub fn new(config_system: &proj_config::ConfigSystem, startup_logger: &mut util::StartupLogger, selected_rom: u32) -> Option<MemorySystem> {

        let cas_rec = match cassette::CassetteRecorder::new(config_system, startup_logger) {
            Some(recorder) => { recorder },
            None => { return None },
        };

        let mut memory_system = MemorySystem {
            ram_chip:          RamChip::new("system ram".to_owned(), config_system.config_items.general_ram_size as u16, RAM_BASE),
            rom_chip:          RomChip::new("system rom".to_owned(), ROM_SIZE, ROM_BASE),
            kbd_mem:           keyboard::KeyboardMemory::new(KBD_BASE),
            vid_mem:           video::VideoMemory::new(config_system.config_items.video_lowercase_mod, VID_BASE),
            cas_rec:           cas_rec,
            nmi_request:       false,
            int_request:       false,

            mode0_int_addr:    0,
            mode2_int_vec:     0,

            logged_messages:   Vec::new(),
            messages_present:  false,
        };

        let rom_choice = match selected_rom {
            1 => { config_system.config_items.general_level_1_rom.clone() },
            2 => { config_system.config_items.general_level_2_rom.clone() },
            3 => { config_system.config_items.general_misc_rom.clone() },
            _ => { panic!("Invalid ROM image selected"); }
        };

        let dummy_rom = include_bytes!("dummy_rom/dummy.rom");
        match rom_choice {
            Some(filename) => {
                // TODO: Perhaps more delicate handling is needed:
                let mut rom_file_path = config_system.config_dir_path.clone();
                rom_file_path.push(filename);

                if !memory_system.rom_chip.load_from_file(&rom_file_path, 0) {
                    memory_system.rom_chip.log_message("Loading the specified rom file failed, resorting to using the built-in dummy rom.".to_owned());
                    memory_system.rom_chip.load_from_buffer(dummy_rom, "built-in dummy rom file", 0);
                }
            },
            None => {
                memory_system.rom_chip.log_message("No system rom file specified, using a buit-in dummy.".to_owned());
                memory_system.rom_chip.load_from_buffer(dummy_rom, "built-in dummy rom file", 0);
            },
        }

        Some(memory_system)
    }

    // External peripherals may detect reti instructions and use them to
    // implement daisy-chaining.
    //
    // This routine gets called when the CPU encounters a reti instruction.
    //
    pub fn reti_notify(&mut self) {
        // Currently, no device needs reti notification.
    }
}

impl MemIO for MemorySystem {
    fn read_byte(&mut self, addr: u16, cycle_timestamp: u32) -> u8 {
        if addr >= RAM_BASE && addr <= (RAM_BASE + ((self.ram_chip.data.len() as u16) - 1)) {
            self.ram_chip.read_byte(addr - RAM_BASE, cycle_timestamp)
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.read_byte(addr - ROM_BASE, cycle_timestamp)
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.read_byte(addr - KBD_BASE, cycle_timestamp)
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.read_byte(addr - VID_BASE, cycle_timestamp)
        } else {
            self.log_message(format!("Warning: Failed read: Address 0x{:04X} doesn't belong to any installed device.", addr));

            // Dunno if this is so for the TRS-80, but in TTL, one would assume
            // that the state of high impedance (neither log. 0 nor 1) would be
            // interpreted as a log. 1
            0xFF
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32) {
        if addr >= RAM_BASE && addr <= (RAM_BASE + ((self.ram_chip.data.len() as u16) - 1)) {
            self.ram_chip.write_byte(addr - RAM_BASE, val, cycle_timestamp);
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.write_byte(addr - ROM_BASE, val, cycle_timestamp);
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.write_byte(addr - KBD_BASE, val, cycle_timestamp);
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.write_byte(addr - VID_BASE, val, cycle_timestamp);
        } else {
            self.log_message(format!("Warning: Failed write of 0x{:02X}: Address 0x{:04X} doesn't belong to any installed device.", val, addr));
        }
    }
}

impl PeripheralIO for MemorySystem {
    fn peripheral_read_byte(&mut self, addr: u16, cycle_timestamp: u32) -> u8 {
        let port: u8 = (addr & 0x00FF) as u8;

        if port == 0xff {
            let mut val = self.cas_rec.peripheral_read_byte(addr, cycle_timestamp);
            if !self.vid_mem.modesel {
                val &= 0b1011_1111
            }

            val
        } else {
            self.log_message(format!("Warning: Failed read: Port 0x{:02X} doesn't belong to any installed peripheral device.", port));

            0xFF
        }
    }
    fn peripheral_write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32) {
        let port: u8 = (addr & 0x00FF) as u8;

        if port == 0xff {
            self.vid_mem.modesel = (val & 0b0000_1000) != 0;
            self.cas_rec.peripheral_write_byte(addr, val, cycle_timestamp);
        } else {
            self.log_message(format!("Warning: Failed write of 0x{:02X}: Port 0x{:02X} doesn't belong to any installed peripheral device.", val, port));
        }
    }
}

impl MessageLogging for MemorySystem {
    fn log_message(&mut self, message: String) {
        self.logged_messages.push(message);
        self.messages_present = true;
    }
    fn messages_available(&self) -> bool {
        self.messages_present
            || self.ram_chip.messages_available()
            || self.rom_chip.messages_available()
            || self.kbd_mem.messages_available()
            || self.vid_mem.messages_available()
            || self.cas_rec.messages_available()
    }
    fn collect_messages(&mut self) -> Vec<String> {
        let mut logged_thus_far: Vec<String> = self.logged_messages.drain(..).collect();
        logged_thus_far.append(&mut self.ram_chip.collect_messages());
        logged_thus_far.append(&mut self.rom_chip.collect_messages());
        logged_thus_far.append(&mut self.kbd_mem.collect_messages());
        logged_thus_far.append(&mut self.vid_mem.collect_messages());
        logged_thus_far.append(&mut self.cas_rec.collect_messages());
        self.messages_present = false;

        logged_thus_far
    }
}
