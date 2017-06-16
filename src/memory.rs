// Copyright (c) 2017 Marek Benc <dusxmt@gmx.com>
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

use std::fmt;
use std::io::prelude::*;
use std::fs;
use std::path;

use proj_config;
use keyboard;
use video;
use cassette;

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
    fn read_byte(&self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, val: u8);

    fn read_word(&self, addr: u16) -> u16 {
        let lsb = self.read_byte(addr);
        let msb = self.read_byte(addr + 1);

        ((msb as u16) << 8) | (lsb as u16)
    }

    fn write_word(&mut self, addr: u16, val: u16) {
        self.write_byte(addr, (val & 0xff) as u8);
        self.write_byte(addr + 1, ((val >> 8) & 0xff) as u8);
    }
}

// A peripheral device is very similar, except that it also gets mutable access
// to itself when being read from, and it also gets timing information.
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

// Write a hexdump of the given data buffer into the formatter `f`.
fn hexdump(f: &mut fmt::Formatter, data: &[u8]) -> fmt::Result {
    let mut row_iter:    usize;
    let mut column_iter: usize;

    row_iter = 0;
    while row_iter < data.len() {
        try!(write!(f, "    {:04X}:", row_iter));
        column_iter = 0;
        while (row_iter + column_iter) < data.len() && column_iter < 16 {
            try!(write!(f, " {:02X}", data[row_iter + column_iter]));
            column_iter += 0x1;
        }
        try!(write!(f, "\n"));
        row_iter += 0x10;
    }
    Ok(())
}

// A RAM memory chip:
pub struct RamChip {
    size: u16,
    data: Box<[u8]>,
}

impl RamChip {
    // Create a new ram chip.
    pub fn new(size: u16) -> RamChip {
        RamChip {
            size: size,
            data: vec![0; size as usize].into_boxed_slice(),
        }
    }
}

impl MemIO for RamChip {
    fn read_byte(&self, addr: u16) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for the ram chip.\n", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize] = val;
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for the ram chip.\n", addr);
        }
    }
}

impl fmt::Debug for RamChip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "RamChip {{\n    Size of the chip: {} (0x{:04X}),\n\n    Hex dump:\n",
            self.size, self.size));
        try!(hexdump(f, &self.data));
        try!(write!(f, "}}"));

        Ok(())
    }
}

// A ROM memory chip:
pub struct RomChip {
    data: Box<[u8]>,
}

impl RomChip {
    // Create a new rom chip.
    pub fn new(data: Box<[u8]>) -> RomChip {
        RomChip {
            data: data,
        }
    }
    // Create a rom chip containing data from the given buffer.
    pub fn new_from_slice(source: &[u8], chip_size: u16) -> RomChip {
        assert!(source.len() <= (chip_size as usize));

        let mut buffer = Vec::with_capacity(chip_size as usize);
        for iter in 0..source.len() {
            buffer.push(source[iter]);
        }
        if buffer.len() < (chip_size as usize) {
            buffer.resize(chip_size as usize, 0xff);
        }
        RomChip::new(buffer.into_boxed_slice())
    }

    // Create a rom chip containing data from the given file.
    // The file's length must be less than or equal to the `chip_size`.
    pub fn new_from_file(path: &path::Path, chip_size: u16)
        -> Option<RomChip> {

        let mut file = match fs::File::open(path) {
            Ok(file) => { file },
            Err(error) => {
                println!("Failed to create the rom chip from `{}': File opening error: {}.", path.display(), error);
                return None;
            },
        };
        let mut buffer = Vec::with_capacity(chip_size as usize);

        match file.read_to_end(&mut buffer) {
            Ok(read_len) => {
                if read_len <= (chip_size as usize) {
                    if read_len < (chip_size as usize) {
                        buffer.resize(chip_size as usize, 0xff);
                    }
                    Some(RomChip::new(buffer.into_boxed_slice()))
                } else {
                    println!("Failed to create the rom chip from `{}': The rom file is too large, expected at most {} (0x{:04X}) bytes, got {} bytes.", path.display(), chip_size, chip_size, read_len);
                    None
                }
            },
            Err(error) => {
                println!("Failed to create the rom chip from `{}': IO Error: {}.", path.display(), error);
                None
            },
        }
    }
}

impl MemIO for RomChip {
    fn read_byte(&self, addr: u16) -> u8 {
        if (addr as usize) < self.data.len() {
            self.data[addr as usize]
        } else {
            panic!("Failed read: Address 0x{:04X} is invalid for the rom chip.\n", addr);
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if (addr as usize) < self.data.len() {
            println!("Warning: Failed write of 0x{:02X}: Invalid operation for the rom chip.", val);
        } else {
            panic!("Failed write: Address 0x{:04X} is invalid for the rom chip.\n", addr);
        }
    }
}

impl fmt::Debug for RomChip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "RomChip {{\n    Size of the chip: {} (0x{:04X}),\n\n    Hex dump:\n",
            self.data.len(), self.data.len()));
        try!(hexdump(f, &self.data));
        try!(write!(f, "}}"));

        Ok(())
    }
}

pub struct MemorySystem {
    pub ram_chip: RamChip,
    pub rom_chip: RomChip,
    pub kbd_mem:  keyboard::KeyboardMemory,
    pub vid_mem:  video::VideoMemory,

    pub cas_rec:  cassette::CassetteRecorder,
}

impl MemorySystem {
    pub fn new(config_system: &proj_config::ConfigSystem, selected_rom: u32) -> Option<MemorySystem> {

        let rom_choice = match selected_rom {
            1 => { config_system.config_items.general_level_1_rom.clone() },
            2 => { config_system.config_items.general_level_2_rom.clone() },
            3 => { config_system.config_items.general_misc_rom.clone() },
            _ => { panic!("Invalid ROM image selected."); }
        };

        let rom = match rom_choice {
            Some(filename) => {
                // TODO: Perhaps more delicate handling is needed:
                let mut rom_file_path = config_system.config_dir_path.clone();
                rom_file_path.push(filename);

                match RomChip::new_from_file(&rom_file_path, ROM_SIZE) {
                    Some(chip) => { chip },
                    None => { return None },
                }
            },
            None => {
                // If no rom file is explicitly specified, use the built-in
                // dummy rom file:
                let dummy_rom = include_bytes!("dummy_rom/dummy.rom");

                println!("No system rom file specified, using a buit-in dummy.");
                RomChip::new_from_slice(dummy_rom, ROM_SIZE)
            },
        };
        let cas_rec = match cassette::CassetteRecorder::new(config_system) {
            Some(recorder) => { recorder },
            None => { return None },
        };

        //println!("{:?}", rom);

        Some(MemorySystem {
            ram_chip:       RamChip::new(config_system.config_items.general_ram_size as u16),
            rom_chip:       rom,
            kbd_mem:        keyboard::KeyboardMemory::new(),
            vid_mem:        video::VideoMemory::new(config_system.config_items.video_lowercase_mod),
            cas_rec:        cas_rec,
        })
    }
}

impl MemIO for MemorySystem {
    fn read_byte(&self, addr: u16) -> u8 {
        if addr >= RAM_BASE && addr <= (RAM_BASE + (self.ram_chip.size - 1)) {
            self.ram_chip.read_byte(addr - RAM_BASE)
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.read_byte(addr - ROM_BASE)
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.read_byte(addr - KBD_BASE)
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.read_byte(addr - VID_BASE)
        } else {
            println!("Warning: Failed read: Address 0x{:04X} doesn't belong to any installed device.", addr);

            // Dunno if this is so for the TRS-80, but in TTL, one would assume
            // that the state of high impedance (neither log. 0 nor 1) would be
            // interpreted as a log. 1
            0xFF
        }
    }
    fn write_byte(&mut self, addr: u16, val: u8) {
        if addr >= RAM_BASE && addr <= (RAM_BASE + (self.ram_chip.size - 1)) {
            self.ram_chip.write_byte(addr - RAM_BASE, val);
        } else if addr >= ROM_BASE && addr <= (ROM_BASE + (ROM_SIZE - 1)) {
            self.rom_chip.write_byte(addr - ROM_BASE, val);
        } else if addr >= KBD_BASE && addr <= (KBD_BASE + (KBD_SIZE - 1)) {
            self.kbd_mem.write_byte(addr - KBD_BASE, val);
        } else if addr >= VID_BASE && addr <= (VID_BASE + (VID_SIZE - 1)) {
            self.vid_mem.write_byte(addr - VID_BASE, val);
        } else {
            println!("Warning: Failed write of 0x{:02X}: Address 0x{:04X} doesn't belong to any installed device.", val, addr);
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
            println!("Warning: Failed read: Port 0x{:02X} doesn't belong to any installed peripheral device.", port);

            0xFF
        }
    }
    fn peripheral_write_byte(&mut self, addr: u16, val: u8, cycle_timestamp: u32) {
        let port: u8 = (addr & 0x00FF) as u8;

        if port == 0xff {
            self.vid_mem.modesel = (val & 0b0000_1000) != 0;
            self.cas_rec.peripheral_write_byte(addr, val, cycle_timestamp);
        } else {
            println!("Warning: Failed write of 0x{:02X}: Port 0x{:02X} doesn't belong to any installed peripheral device.", val, port);
        }
    }
}
