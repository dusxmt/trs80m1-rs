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

use log::{info, warn, error};

use std::io::prelude::*;
use std::fs;
use std::path;

use crate::keyboard;
use crate::video;
use crate::cassette;


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

// Combined cassette and "mode select" IO port:
pub const CAS_MODESEL_BASE: u16 = 0xff;

// A memory device is one that implements the read and write operations.
pub trait MemIO {
    fn read_byte(&mut self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, val: u8);

    fn read_word(&mut self, addr: u16) -> u16 {
        let lsb = self.read_byte(addr);
        let msb = self.read_byte(addr + 1);

        ((msb as u16) << 8) | (lsb as u16)
    }

    fn write_word(&mut self, addr: u16, val: u16) {
        self.write_byte(addr, (val & 0xff) as u8);
        self.write_byte(addr + 1, ((val >> 8) & 0xff) as u8);
    }
}

// A peripheral device is very similar, except it's accessed differently.
pub trait PeripheralIO {
    fn peripheral_read_byte(&mut self, addr: u16) -> u8;
    fn peripheral_write_byte(&mut self, addr: u16, val: u8);

    fn peripheral_read_word(&mut self, addr: u16) -> u16 {
        let lsb = self.peripheral_read_byte(addr);
        let msb = self.peripheral_read_byte(addr + 1);

        ((msb as u16) << 8) | (lsb as u16)
    }

    fn peripheral_write_word(&mut self, addr: u16, val: u16) {
        self.peripheral_write_byte(addr, (val & 0xff) as u8);
        self.peripheral_write_byte(addr + 1, ((val >> 8) & 0xff) as u8);
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
}

impl RamChip {
    // Create a new ram chip.
    pub fn new(id: String, size: u16, start_addr: u16) -> RamChip {

        let chip = RamChip {
            id:   id,
            data: vec![0; size as usize].into_boxed_slice(),
        };

        if (size % 1024) == 0 {
            info!("Created ram chip `{}', starting address: 0x{:04X}, size: {}K.", chip.id, start_addr, size / 1024)
        } else {
            info!("Created ram chip `{}', starting address: 0x{:04X}, size: {} bytes.", chip.id, start_addr, size)
        };

        chip
    }
    pub fn change_size(&mut self, new_size: u16) {
        let mut data_vec = self.data.clone().into_vec();
        data_vec.resize(new_size as usize, self.default_value());
        self.data = data_vec.into_boxed_slice();
    }
}

impl MemIO for RamChip {
    fn read_byte(&mut self, addr: u16) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address offset 0x{:04X} is invalid for RAM chip `{}'", addr, self.id);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize] = val;
        } else {
            panic!("Failed write: Address offset 0x{:04X} is invalid for RAM chip `{}'", addr, self.id);
        }
    }
}

// A ROM memory chip:
pub struct RomChip {
    id:    String,
    data:  Box<[u8]>,
}

impl RomChip {
    // Create a new rom chip.
    pub fn new(id: String, size: u16, start_addr: u16) -> RomChip {
        let chip = RomChip {
            id:   id,
            data: vec![0xFF; size as usize].into_boxed_slice(),
        };

        if (size % 1024) == 0 {
            info!("Created rom chip `{}', starting address: 0x{:04X}, size: {}K.", chip.id, start_addr, size / 1024)
        } else {
            info!("Created rom chip `{}', starting address: 0x{:04X}, size: {} bytes.", chip.id, start_addr, size)
        };

        chip
    }
}

impl MemIO for RomChip {
    fn read_byte(&mut self, addr: u16) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address offset 0x{:04X} is invalid for ROM chip `{}'", addr, self.id);
        }
    }
    fn write_byte(&mut self, addr: u16, _val: u8) {
        if (addr as usize) < self.data.len() {
            let id = self.id.clone();
            warn!("Failed write: Invalid operation for ROM chip `{}'.", id);
        } else {
            panic!("Failed write: Address offset 0x{:04X} is invalid for ROM chip `{}'", addr, self.id);
        }
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

impl<T: MemoryChip> MemoryChipOps for T {

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
        info!("The memory chip `{}' was wiped.", id);
    }

    // Load a file into a memory chip:
    fn load_from_file<P: AsRef<path::Path>>(&mut self, path_in: P, offset: u16) -> bool {
        let id   = self.chip_id().to_owned();
        let path = path_in.as_ref() as &path::Path;

        if (offset as usize) >= self.chip_data_mut().len() {
            error!("Failed to load the file `{}' into `{}', offset 0x{:04X}: Offset out of range.", path.display(), id, offset);

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
                        info!("Loaded {} bytes from `{}' into `{}', offset 0x{:04X}.", load_count, path.display(), id, offset);
                        if read_len > load_count {
                            warn!("{} bytes from `{}' ({} bytes large) didn't fit into `{}' at offset 0x{:04X}.", read_len - load_count, path.display(), read_len, id, offset);
                        }
                        true
                    },
                    Err(error) => {
                        error!("Failed to load the file `{}' into `{}', offset 0x{:04X}: File reading error: {}.", path.display(), id, offset, error);
                        false
                    },
                }
            },
            Err(error) => {
                error!("Failed to load the file `{}' into `{}', offset 0x{:04X}: File opening error: {}.", path.display(), id, offset, error);
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
        info!("Loaded {} bytes from `{}' into `{}', offset 0x{:04X}.", load_count, input_name, id, offset);
        if input_len > load_count {
            warn!("{} bytes from `{}' ({} bytes large) didn't fit into `{}' at offset 0x{:04X}.", input_len - load_count, input_name, input_len, id, offset);
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
//        write!(f, "    {:04X}:", row_iter)?;
//        column_iter = 0;
//        while (row_iter + column_iter) < data.len() && column_iter < 16 {
//            write!(f, " {:02X}", data[row_iter + column_iter])?;
//            column_iter += 0x1;
//        }
//        write!(f, "\n")?;
//        row_iter += 0x10;
//    }
//    Ok(())
//}
//
//impl<T: MemoryChip> fmt::Debug for T {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        let chip_id = self.chip_id().to_owned();
//        let data    = self.chip_data();
//        write!(f, "MemoryChip {{\n    id: `{}',\n    Size of the chip: {} (0x{:04X}),\n\n    Hex dump:\n",
//            chip_id, data.len(), data.len())?;
//        hexdump(f, data)?;
//        write!(f, "}}")?;
//
//        Ok(())
//    }
//}


pub struct MemorySystem {
    pub ram_chip: RamChip,
    pub rom_chip: RomChip,
    pub kbd_mem:  keyboard::KeyboardMemory,
    pub vid_mem:  video::VideoMemory,

    pub cas_rec:  cassette::CassetteIO,

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
}

impl MemorySystem {
    pub fn new(ram_size: u16, rom_choice: Option<path::PathBuf>, lowercase_mod: bool) -> MemorySystem {

        let mut memory_system = MemorySystem {
            ram_chip:          RamChip::new("system ram".to_owned(), ram_size, RAM_BASE),
            rom_chip:          RomChip::new("system rom".to_owned(), ROM_SIZE, ROM_BASE),
            kbd_mem:           keyboard::KeyboardMemory::new(KBD_BASE),
            vid_mem:           video::VideoMemory::new(lowercase_mod, VID_BASE),
            cas_rec:           cassette::CassetteIO::new(),
            nmi_request:       false,
            int_request:       false,

            mode0_int_addr:    0,
            mode2_int_vec:     0,
        };
        memory_system.load_system_rom(rom_choice);

        memory_system
    }
    pub fn power_off(&mut self) {
        self.ram_chip.wipe();
        self.nmi_request = false;
        self.int_request = false;
    }
    pub fn load_system_rom(&mut self, rom_choice: Option<path::PathBuf>) {

        let dummy_rom = include_bytes!("dummy_rom/dummy.rom");
        match rom_choice {
            Some(rom_file_path) => {
                if !self.rom_chip.load_from_file(&rom_file_path, 0) {
                    warn!("Loading the specified rom file failed, resorting to using the built-in dummy rom.");
                    self.rom_chip.load_from_buffer(dummy_rom, "built-in dummy rom file", 0);
                }
            },
            None => {
                warn!("No system rom file specified, using a buit-in dummy.");
                self.rom_chip.load_from_buffer(dummy_rom, "built-in dummy rom file", 0);
            },
        }
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
    fn read_byte(&mut self, addr: u16) -> u8 {
        if addr >= RAM_BASE && addr <= (RAM_BASE + ((self.ram_chip.data.len() as u16) - 1)) {
            self.ram_chip.read_byte(addr - RAM_BASE)
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.read_byte(addr - ROM_BASE)
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.read_byte(addr - KBD_BASE)
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.read_byte(addr - VID_BASE)
        } else {
            warn!("Failed read: Address 0x{:04X} doesn't belong to any installed device.", addr);

            // Dunno if this is so for the TRS-80, but in TTL, one would assume
            // that the state of high impedance (neither log. 0 nor 1) would be
            // interpreted as a log. 1
            0xFF
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if addr >= RAM_BASE && addr <= (RAM_BASE + ((self.ram_chip.data.len() as u16) - 1)) {
            self.ram_chip.write_byte(addr - RAM_BASE, val);
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.write_byte(addr - ROM_BASE, val);
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.write_byte(addr - KBD_BASE, val);
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.write_byte(addr - VID_BASE, val);
        } else {
            warn!("Failed write of 0x{:02X}: Address 0x{:04X} doesn't belong to any installed device.", val, addr);
        }
    }
}

impl PeripheralIO for MemorySystem {
    fn peripheral_read_byte(&mut self, addr: u16) -> u8 {
        let port = addr & 0x00FF;

        if port == CAS_MODESEL_BASE {
            let mut val = self.cas_rec.peripheral_read_byte(port - CAS_MODESEL_BASE);
            if !self.vid_mem.modesel {
                val &= 0b1011_1111
            }

            val
        } else {
            warn!("Failed read: Port 0x{:02X} doesn't belong to any installed peripheral device.", port);

            0xFF
        }
    }
    fn peripheral_write_byte(&mut self, addr: u16, val: u8) {
        let port = addr & 0x00FF;

        if port == CAS_MODESEL_BASE {
            self.vid_mem.modesel = (val & 0b0000_1000) != 0;
            self.cas_rec.peripheral_write_byte(port - CAS_MODESEL_BASE, val);
        } else {
            warn!("Failed write of 0x{:02X}: Port 0x{:02X} doesn't belong to any installed peripheral device.", val, port);
        }
    }
}
