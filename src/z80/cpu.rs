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


use memory;
use memory::MemIO;
use z80::instructions;

// This is a software implementation of the Zilog Z80.


// Interrupt modes:
pub enum InterruptMode {
   Mode0,
   Mode1,
   Mode2,
   ModeUndefined,
}

// Constants:
pub const RESET_EXEC_START:      u16 = 0x0000;
pub const MODE1_INT_VEC:         u16 = 0x0038;
pub const MODE2_INT_VEC_HIGH:    u8  = 0x00;

// Flags register contents description:
pub const FLAG_SIGN:             u8  = 0b1000_0000;
pub const FLAG_ZERO:             u8  = 0b0100_0000;
pub const FLAG_UNDOC_Y:          u8  = 0b0010_0000;
pub const FLAG_HALF_CARRY:       u8  = 0b0001_0000;
pub const FLAG_UNDOC_X:          u8  = 0b0000_1000;
pub const FLAG_PARITY_OVERFLOW:  u8  = 0b0000_0100;
pub const FLAG_ADD_SUB:          u8  = 0b0000_0010;
pub const FLAG_CARRY:            u8  = 0b0000_0001;

// Flags structure:
#[derive(Clone)]
#[derive(Debug)]
pub struct Z80Flags {
    pub sign:             bool,
    pub zero:             bool,
    pub undoc_y:          bool,
    pub half_carry:       bool,
    pub undoc_x:          bool,
    pub parity_overflow:  bool,
    pub add_sub:          bool,
    pub carry:            bool,
}

// Registers:
#[derive(Debug)]
pub struct Z80Regs {
    pub pc: u16,
    pub i:  u8,
    pub r:  u8,
    pub sp: u16,
    pub ix: u16,
    pub iy: u16,

    pub a:  u8,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub flags: Z80Flags,

    pub a_prime:  u8,
    pub bc_prime: u16,
    pub de_prime: u16,
    pub hl_prime: u16,
    pub flags_prime: Z80Flags,
}
pub struct CPU {
    pub regs:             Z80Regs,
    pub halted:           bool,
    pub im:               InterruptMode,
    pub iff1:             bool,
    pub iff2:             bool,
    pub added_delay:      u32,
    pub cycle_overshoot:  u32,
    pub cycle_timestamp:  u32,
    current_inst:        &'static instructions::Instruction,
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            regs: Z80Regs {
                pc:       RESET_EXEC_START,
                i:        MODE2_INT_VEC_HIGH,
                r:        0xff,
                sp:       0xffff,
                ix:       0xffff,
                iy:       0xffff,

                a:        0xff,
                bc:       0xffff,
                de:       0xffff,
                hl:       0xffff,

                flags: Z80Flags {
                    sign:             true,
                    zero:             true,
                    undoc_y:          true,
                    half_carry:       true,
                    undoc_x:          true,
                    parity_overflow:  true,
                    add_sub:          true,
                    carry:            true,
                },

                a_prime:  0xff,
                bc_prime: 0xffff,
                de_prime: 0xffff,
                hl_prime: 0xffff,

                flags_prime: Z80Flags {
                    sign:             true,
                    zero:             true,
                    undoc_y:          true,
                    half_carry:       true,
                    undoc_x:          true,
                    parity_overflow:  true,
                    add_sub:          true,
                    carry:            true,
                },
            },
            halted:          false,
            im:              InterruptMode::Mode0,
            iff1:            false,
            iff2:            false,
            added_delay:     0,
            cycle_overshoot: 0,
            cycle_timestamp: 0,
            current_inst: &instructions::INSTRUCTION_SET.nop_1,
        }
    }
    // Execute at least `cycles_to_exec - self.cycle_overshoot` of machine
    // cycles. If more cycles are executed (we overshat), compensate for it
    // on the next invocation of this method.
    pub fn exec(&mut self, cycles_to_exec: u32, memory_system: &mut memory::MemorySystem) {
        let cycles_to_exec_comp: i32 = (cycles_to_exec as i32) - (self.cycle_overshoot as i32);
        let mut executed_cycles: i32 = 0;
        let halted_before = self.halted;

        while executed_cycles < cycles_to_exec_comp {
            if self.halted {
                executed_cycles += 4;
            } else {
                self.regs.i = self.regs.i.wrapping_add(1);

                self.current_inst = instructions::load_instruction(self.regs.pc, &memory_system);
                self.added_delay = 0;

                (self.current_inst.execute)(self, memory_system);
                self.cycle_timestamp = self.cycle_timestamp.wrapping_add(self.current_inst.clock_cycles);
                self.cycle_timestamp = self.cycle_timestamp.wrapping_add(self.added_delay);

                executed_cycles += self.current_inst.clock_cycles as i32;
                executed_cycles += self.added_delay as i32;
            }
        }
        self.cycle_overshoot = (executed_cycles - cycles_to_exec_comp) as u32;
        //println!("[{:10}]: {:10} CPU cycles requested, executed {:10}.", self.cycle_timestamp, cycles_to_exec, executed_cycles);


        // Inform the user tha the CPU is halted.
        if !halted_before && self.halted {
            println!("Warning: The CPU is halted, and interrupts are not yet implemented. The emulator is stuck with no way of recovery.");
        }
    }
}
