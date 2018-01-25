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


use memory;
use memory::MemIO;
use util::MessageLogging;
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
pub const NMI_VEC:               u16 = 0x0066;
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
#[derive(Clone, Debug, Default)]
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
#[derive(Debug, Default)]
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
    pub int_enabled:      bool,
    pub added_delay:      u32,
    pub cycle_overshoot:  u32,
    pub cycle_timestamp:  u32,
    current_inst:        &'static instructions::Instruction,

    logged_messages:      Vec<String>,
    messages_present:     bool,
}

impl CPU {
    pub fn new() -> CPU {
        let mut cpu = CPU {
                          regs:            Z80Regs::default(),
                          halted:          true,
                          im:              InterruptMode::Mode0,
                          iff1:            false,
                          iff2:            false,
                          int_enabled:     false,
                          added_delay:     0,
                          cycle_overshoot: 0,
                          cycle_timestamp: 0,
                          current_inst:    &instructions::INSTRUCTION_SET.nop_1,

                          logged_messages:  Vec::new(),
                          messages_present: false,
                      };

        cpu.log_message("Created an emulated Z80 CPU.".to_owned());
        cpu
    }
    // Put the CPU into a well-defined state:
    pub fn full_reset(&mut self) {
        self.regs.pc  = RESET_EXEC_START;
        self.regs.i   = MODE2_INT_VEC_HIGH;
        self.regs.r   = 0xff;
        self.regs.sp  = 0xffff;
        self.regs.ix  = 0xffff;
        self.regs.iy  = 0xffff;

        self.regs.a   = 0xff;
        self.regs.bc  = 0xffff;
        self.regs.de  = 0xffff;
        self.regs.hl  = 0xffff;

        self.regs.flags.sign            = true;
        self.regs.flags.zero            = true;
        self.regs.flags.undoc_y         = true;
        self.regs.flags.half_carry      = true;
        self.regs.flags.undoc_x         = true;
        self.regs.flags.parity_overflow = true;
        self.regs.flags.add_sub         = true;
        self.regs.flags.carry           = true;

        self.regs.a_prime   = 0xff;
        self.regs.bc_prime  = 0xffff;
        self.regs.de_prime  = 0xffff;
        self.regs.hl_prime  = 0xffff;

        self.regs.flags_prime.sign            = true;
        self.regs.flags_prime.zero            = true;
        self.regs.flags_prime.undoc_y         = true;
        self.regs.flags_prime.half_carry      = true;
        self.regs.flags_prime.undoc_x         = true;
        self.regs.flags_prime.parity_overflow = true;
        self.regs.flags_prime.add_sub         = true;
        self.regs.flags_prime.carry           = true;

        self.halted          = false;
        self.im              = InterruptMode::Mode0;
        self.iff1            = false;
        self.iff2            = false;
        self.int_enabled     = false;
        self.added_delay     = 0;
        self.cycle_overshoot = 0;
        self.cycle_timestamp = 0;
        self.current_inst    = &instructions::INSTRUCTION_SET.nop_1;
    }

    // Put the CPU into a post-reset state:
    pub fn reset(&mut self) {
        self.regs.pc         = RESET_EXEC_START;
        self.regs.i          = MODE2_INT_VEC_HIGH;

        self.halted          = false;
        self.iff1            = false;
        self.iff2            = false;

        self.added_delay     = 0;
        self.cycle_overshoot = 0;
        self.cycle_timestamp = 0;
        self.current_inst    = &instructions::INSTRUCTION_SET.nop_1;
    }

    // Disable interrupts and halt the CPU:
    pub fn power_off(&mut self) {
        self.halted      = true;
        self.iff1        = false;
        self.iff2        = false;
    }

    // Perform a non-maskable interrupt:
    fn perform_nmi(&mut self, memory: &mut memory::MemorySystem) -> i32 {
        self.iff2 = self.iff1;
        self.iff1 = false;

        stack_push_16bit!(self.regs, memory, self.regs.pc, self.cycle_timestamp);
        self.regs.pc = NMI_VEC;

        // The NMI acts as a reset instruction, which takes 11 T cycles:
        11
    }

    // Perform a maskable interrupt:
    fn perform_int(&mut self, memory: &mut memory::MemorySystem) -> i32 {
        self.iff2 = false;
        self.iff1 = false;

        match self.im {
            InterruptMode::Mode0 => {
                // Even though in mode 0, the interrupting peripheral can send
                // an arbitrary instruction to the CPU, in practice it's most
                // often a reset instruction, and that's what this code assumes.
                //
                // If more specific needs for the interrupt mode 0 arise, it
                // should be easy enough to expand the code to perform the
                // operations of the needed instructions.  I decided to avoid
                // having a pointer to an opcode routine from the instructions
                // module, because those routines assume that they can load
                // parts of the instruction from main memory, but we want to
                // load the instruction with all of itss parameters form the
                // interrupting peripheral.
                //
                stack_push_16bit!(self.regs, memory, self.regs.pc, self.cycle_timestamp);
                self.regs.pc = memory.mode0_int_addr;

                // According to the Z80 Family CPU User Manual:
                //
                // The number of clock cycles necessary to execute this
                // instruction is two more than the normal number for the
                // instruction.  This occurs because the CPU automatically
                // adds two wait states to an Interrupt response cycle to
                // allow sufficient time to implement an external daisy-chain
                // for priority control.
                //
                // Hence, we add two T cycles here:
                11 + 2
            },
            InterruptMode::Mode1 => {
                stack_push_16bit!(self.regs, memory, self.regs.pc, self.cycle_timestamp);
                self.regs.pc = MODE1_INT_VEC;

                // Mode 1 maskable interrupts act as a reset instruction, which takes 11 T cycles:
                11
            },
            InterruptMode::Mode2 => {
                let int_vec_index = memory.mode2_int_vec & 0xFE;
                stack_push_16bit!(self.regs, memory, self.regs.pc, self.cycle_timestamp.wrapping_add(7));

                let int_vec_addr = compose_16bit_from_8bit!(self.regs.i, int_vec_index);
                self.regs.pc = memory.read_word(int_vec_addr, self.cycle_timestamp.wrapping_add(7 + 6));

                // According to the Z80 Family CPU User Manual:
                //
                // This mode of response requires 19 clock periods to complete
                // (seven to fetch the lower eight bits from the interrupting
                // device, six to save the program counter, and six to obtain
                // the jump address).
                //
                7 + 6 + 6
            },
            InterruptMode::ModeUndefined => {
                self.log_message("I haven't got a clue on how to service interrupts in the 0/1 mode...".to_owned());
                4
            },
        }
    }

    // Execute at least `cycles_to_exec - self.cycle_overshoot` of machine
    // cycles. If more cycles are executed (we overshat), compensate for it
    // on the next invocation of this method.
    pub fn exec(&mut self, cycles_to_exec: u32, memory_system: &mut memory::MemorySystem) {
        let cycles_to_exec_comp: i32 = (cycles_to_exec as i32) - (self.cycle_overshoot as i32);
        let mut executed_cycles: i32 = 0;

        while executed_cycles < cycles_to_exec_comp {

            self.regs.r = (self.regs.r & 0x80) | (self.regs.r.wrapping_add(1) & 0x7F);

            if self.int_enabled && !self.iff1 {
                self.int_enabled = false;
            }

            if memory_system.nmi_request {
                if self.halted {
                    self.halted = false;
                    self.regs.pc += 1;
                }
                let spent_clock_cycles = self.perform_nmi(memory_system);
                executed_cycles += spent_clock_cycles;
                self.cycle_timestamp = self.cycle_timestamp.wrapping_add(spent_clock_cycles as u32);
                memory_system.nmi_request = false;

            } else if memory_system.int_request && self.int_enabled {
                if self.halted {
                    self.halted = false;
                    self.regs.pc += 1;
                }
                let spent_clock_cycles = self.perform_int(memory_system);
                executed_cycles += spent_clock_cycles;
                self.cycle_timestamp = self.cycle_timestamp.wrapping_add(spent_clock_cycles as u32);
                memory_system.int_request = false;

            } else if self.halted {
                // The following check is done in order to ensure that
                // maskable interrupts are only serviced once the instruction
                // following the ei instruction is executed.
                //
                if self.iff1 && !self.int_enabled {
                    self.int_enabled = true;
                }

                executed_cycles += 4;
                self.cycle_timestamp = self.cycle_timestamp.wrapping_add(4);

            } else {
                // The following check is done in order to ensure that
                // maskable interrupts are only serviced once the instruction
                // following the ei instruction is executed.
                //
                if self.iff1 && !self.int_enabled {
                    self.int_enabled = true;
                }

                self.current_inst = instructions::load_instruction(self.regs.pc, memory_system, self.cycle_timestamp);
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
    }
}

impl MessageLogging for CPU {
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
