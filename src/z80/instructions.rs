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

// This file contains an implementation of the Zilog Z80's instruction set.
//
// Because the Z80 is a reasonably simple CPU, it is still within reason
// to implement each of its opcodes as separate routines, and using a set
// of look-up tables to get the corresponding routine for the instruction
// to be executed.
//
// An interesting property of the Z80 is that it has no "illegal instruction"
// exception, illegal instructions are interpreted as NOPs, so any possible
// sequence of bytes is therefore a valid Z80 program (albeit not always
// a useful one).
//
// Most of the undocumented instructions should be implemented (if there are
// missing ones, that means I must've overlooked them - it's a bug worth
// reporting), and execution timing information is also provided (via the
// `clock_cycles` field of a `struct Instruction`, and the `added_delay` field
// of the CPU structure).
//
// The only feature that I know is not authentic is flags handling - any flags
// that are mentioned to be `unknown` in the "Zilog Z80 Family CPU User Manual"
// (UM008004-1204) are left unaltered by this implemenntation.
//
// It might be good to note that I found several inconsistencies and errors in
// the above-mentioned manual, eg. the and/or/xor instructions apparently set
// the P/V flag on "overflow", there is sometimes the occasional non-existent
// R flag mentioned, etc. If you notice any error here that's caused by an
// error in the manual, it's a bug worth reporting as well :)

// Parentheses help code readability, which is especially important here.
#![allow(unused_parens)]

use memory;
use memory::MemIO;
use memory::PeripheralIO;
use util::MessageLogging;
use z80::cpu;

pub struct Instruction {
    pub execute:       fn (&mut cpu::CPU, &mut memory::MemorySystem),
    pub clock_cycles:  u32,
    pub size:          u16, // The size is in there in case I decide to write
}                           // a disassembler for these.

pub struct InstructionSet {
    pub nop_1:    Instruction,
    pub nop_2:    Instruction,
    pub main:     [Instruction; 256],
    pub extended: [Instruction; 96],
    pub bit:      [Instruction; 256],
    pub ix:       [Instruction; 256],
    pub ix_bit:   [Instruction; 256],
    pub iy:       [Instruction; 256],
    pub iy_bit:   [Instruction; 256],
}

pub static INSTRUCTION_SET: InstructionSet = InstructionSet {
    // No-ops:
    nop_1: Instruction {
               execute: inst_nop1,
               clock_cycles: 4,
               size: 1,
           },
    nop_2: Instruction {
               execute: inst_nop2,
               clock_cycles: 8,
               size: 2,
           },
    // Main instructions, mostly derived from the Intel 8080:
    main: [
        /* 00 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 01 */ Instruction {
            execute: inst_ld_bc_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* 02 */ Instruction {
            execute: inst_ld_mem_bc_a,
            clock_cycles: 7,
            size: 1,
        },
        /* 03 */ Instruction {
            execute: inst_inc_bc,
            clock_cycles: 6,
            size: 1,
        },
        /* 04 */ Instruction {
            execute: inst_inc_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 05 */ Instruction {
            execute: inst_dec_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 06 */ Instruction {
            execute: inst_ld_b_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 07 */ Instruction {
            execute: inst_rlca,
            clock_cycles: 4,
            size: 1,
        },
        /* 08 */ Instruction {
            execute: inst_ex_af_af_prime,
            clock_cycles: 4,
            size: 1,
        },
        /* 09 */ Instruction {
            execute: inst_add_hl_bc,
            clock_cycles: 11,
            size: 1,
        },
        /* 0A */ Instruction {
            execute: inst_ld_a_mem_bc,
            clock_cycles: 7,
            size: 1,
        },
        /* 0B */ Instruction {
            execute: inst_dec_bc,
            clock_cycles: 6,
            size: 1,
        },
        /* 0C */ Instruction {
            execute: inst_inc_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 0D */ Instruction {
            execute: inst_dec_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 0E */ Instruction {
            execute: inst_ld_c_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 0F */ Instruction {
            execute: inst_rrca,
            clock_cycles: 4,
            size: 1,
        },
        /* 10 */ Instruction {
            execute: inst_djnz_im8,
            clock_cycles: 8,
            size: 2,
        },
        /* 11 */ Instruction {
            execute: inst_ld_de_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* 12 */ Instruction {
            execute: inst_ld_mem_de_a,
            clock_cycles: 7,
            size: 1,
        },
        /* 13 */ Instruction {
            execute: inst_inc_de,
            clock_cycles: 6,
            size: 1,
        },
        /* 14 */ Instruction {
            execute: inst_inc_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 15 */ Instruction {
            execute: inst_dec_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 16 */ Instruction {
            execute: inst_ld_d_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 17 */ Instruction {
            execute: inst_rla,
            clock_cycles: 4,
            size: 1,
        },
        /* 18 */ Instruction {
            execute: inst_jr_im8,
            clock_cycles: 12,
            size: 2,
        },
        /* 19 */ Instruction {
            execute: inst_add_hl_de,
            clock_cycles: 11,
            size: 1,
        },
        /* 1A */ Instruction {
            execute: inst_ld_a_mem_de,
            clock_cycles: 7,
            size: 1,
        },
        /* 1B */ Instruction {
            execute: inst_dec_de,
            clock_cycles: 6,
            size: 1,
        },
        /* 1C */ Instruction {
            execute: inst_inc_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 1D */ Instruction {
            execute: inst_dec_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 1E */ Instruction {
            execute: inst_ld_e_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 1F */ Instruction {
            execute: inst_rra,
            clock_cycles: 4,
            size: 1,
        },
        /* 20 */ Instruction {
            execute: inst_jr_nz_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 21 */ Instruction {
            execute: inst_ld_hl_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* 22 */ Instruction {
            execute: inst_ld_mem_im16_hl,
            clock_cycles: 16,
            size: 3,
        },
        /* 23 */ Instruction {
            execute: inst_inc_hl,
            clock_cycles: 6,
            size: 1,
        },
        /* 24 */ Instruction {
            execute: inst_inc_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 25 */ Instruction {
            execute: inst_dec_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 26 */ Instruction {
            execute: inst_ld_h_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 27 */ Instruction {
            execute: inst_daa,
            clock_cycles: 4,
            size: 1,
        },
        /* 28 */ Instruction {
            execute: inst_jr_z_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 29 */ Instruction {
            execute: inst_add_hl_hl,
            clock_cycles: 11,
            size: 1,
        },
        /* 2A */ Instruction {
            execute: inst_ld_hl_mem_im16,
            clock_cycles: 16,
            size: 3,
        },
        /* 2B */ Instruction {
            execute: inst_dec_hl,
            clock_cycles: 6,
            size: 1,
        },
        /* 2C */ Instruction {
            execute: inst_inc_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 2D */ Instruction {
            execute: inst_dec_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 2E */ Instruction {
            execute: inst_ld_l_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 2F */ Instruction {
            execute: inst_cpl,
            clock_cycles: 4,
            size: 1,
        },
        /* 30 */ Instruction {
            execute: inst_jr_nc_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 31 */ Instruction {
            execute: inst_ld_sp_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* 32 */ Instruction {
            execute: inst_ld_mem_im16_a,
            clock_cycles: 13,
            size: 3,
        },
        /* 33 */ Instruction {
            execute: inst_inc_sp,
            clock_cycles: 6,
            size: 1,
        },
        /* 34 */ Instruction {
            execute: inst_inc_mem_hl,
            clock_cycles: 11,
            size: 1,
        },
        /* 35 */ Instruction {
            execute: inst_dec_mem_hl,
            clock_cycles: 11,
            size: 1,
        },
        /* 36 */ Instruction {
            execute: inst_ld_mem_hl_im8,
            clock_cycles: 10,
            size: 2,
        },
        /* 37 */ Instruction {
            execute: inst_scf,
            clock_cycles: 4,
            size: 1,
        },
        /* 38 */ Instruction {
            execute: inst_jr_c_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 39 */ Instruction {
            execute: inst_add_hl_sp,
            clock_cycles: 11,
            size: 1,
        },
        /* 3A */ Instruction {
            execute: inst_ld_a_mem_im16,
            clock_cycles: 13,
            size: 3,
        },
        /* 3B */ Instruction {
            execute: inst_dec_sp,
            clock_cycles: 6,
            size: 1,
        },
        /* 3C */ Instruction {
            execute: inst_inc_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 3D */ Instruction {
            execute: inst_dec_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 3E */ Instruction {
            execute: inst_ld_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* 3F */ Instruction {
            execute: inst_ccf,
            clock_cycles: 4,
            size: 1,
        },
        /* 40 */ Instruction {
            execute: inst_ld_b_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 41 */ Instruction {
            execute: inst_ld_b_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 42 */ Instruction {
            execute: inst_ld_b_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 43 */ Instruction {
            execute: inst_ld_b_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 44 */ Instruction {
            execute: inst_ld_b_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 45 */ Instruction {
            execute: inst_ld_b_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 46 */ Instruction {
            execute: inst_ld_b_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 47 */ Instruction {
            execute: inst_ld_b_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 48 */ Instruction {
            execute: inst_ld_c_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 49 */ Instruction {
            execute: inst_ld_c_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 4A */ Instruction {
            execute: inst_ld_c_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 4B */ Instruction {
            execute: inst_ld_c_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 4C */ Instruction {
            execute: inst_ld_c_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 4D */ Instruction {
            execute: inst_ld_c_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 4E */ Instruction {
            execute: inst_ld_c_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 4F */ Instruction {
            execute: inst_ld_c_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 50 */ Instruction {
            execute: inst_ld_d_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 51 */ Instruction {
            execute: inst_ld_d_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 52 */ Instruction {
            execute: inst_ld_d_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 53 */ Instruction {
            execute: inst_ld_d_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 54 */ Instruction {
            execute: inst_ld_d_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 55 */ Instruction {
            execute: inst_ld_d_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 56 */ Instruction {
            execute: inst_ld_d_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 57 */ Instruction {
            execute: inst_ld_d_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 58 */ Instruction {
            execute: inst_ld_e_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 59 */ Instruction {
            execute: inst_ld_e_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 5A */ Instruction {
            execute: inst_ld_e_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 5B */ Instruction {
            execute: inst_ld_e_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 5C */ Instruction {
            execute: inst_ld_e_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 5D */ Instruction {
            execute: inst_ld_e_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 5E */ Instruction {
            execute: inst_ld_e_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 5F */ Instruction {
            execute: inst_ld_e_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 60 */ Instruction {
            execute: inst_ld_h_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 61 */ Instruction {
            execute: inst_ld_h_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 62 */ Instruction {
            execute: inst_ld_h_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 63 */ Instruction {
            execute: inst_ld_h_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 64 */ Instruction {
            execute: inst_ld_h_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 65 */ Instruction {
            execute: inst_ld_h_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 66 */ Instruction {
            execute: inst_ld_h_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 67 */ Instruction {
            execute: inst_ld_h_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 68 */ Instruction {
            execute: inst_ld_l_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 69 */ Instruction {
            execute: inst_ld_l_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 6A */ Instruction {
            execute: inst_ld_l_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 6B */ Instruction {
            execute: inst_ld_l_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 6C */ Instruction {
            execute: inst_ld_l_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 6D */ Instruction {
            execute: inst_ld_l_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 6E */ Instruction {
            execute: inst_ld_l_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 6F */ Instruction {
            execute: inst_ld_l_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 70 */ Instruction {
            execute: inst_ld_mem_hl_b,
            clock_cycles: 7,
            size: 1,
        },
        /* 71 */ Instruction {
            execute: inst_ld_mem_hl_c,
            clock_cycles: 7,
            size: 1,
        },
        /* 72 */ Instruction {
            execute: inst_ld_mem_hl_d,
            clock_cycles: 7,
            size: 1,
        },
        /* 73 */ Instruction {
            execute: inst_ld_mem_hl_e,
            clock_cycles: 7,
            size: 1,
        },
        /* 74 */ Instruction {
            execute: inst_ld_mem_hl_h,
            clock_cycles: 7,
            size: 1,
        },
        /* 75 */ Instruction {
            execute: inst_ld_mem_hl_l,
            clock_cycles: 7,
            size: 1,
        },
        /* 76 */ Instruction {
            execute: inst_halt,
            clock_cycles: 4,
            size: 1,
        },
        /* 77 */ Instruction {
            execute: inst_ld_mem_hl_a,
            clock_cycles: 7,
            size: 1,
        },
        /* 78 */ Instruction {
            execute: inst_ld_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 79 */ Instruction {
            execute: inst_ld_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 7A */ Instruction {
            execute: inst_ld_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 7B */ Instruction {
            execute: inst_ld_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 7C */ Instruction {
            execute: inst_ld_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 7D */ Instruction {
            execute: inst_ld_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 7E */ Instruction {
            execute: inst_ld_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 7F */ Instruction {
            execute: inst_ld_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 80 */ Instruction {
            execute: inst_add_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 81 */ Instruction {
            execute: inst_add_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 82 */ Instruction {
            execute: inst_add_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 83 */ Instruction {
            execute: inst_add_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 84 */ Instruction {
            execute: inst_add_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 85 */ Instruction {
            execute: inst_add_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 86 */ Instruction {
            execute: inst_add_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 87 */ Instruction {
            execute: inst_add_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 88 */ Instruction {
            execute: inst_adc_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 89 */ Instruction {
            execute: inst_adc_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 8A */ Instruction {
            execute: inst_adc_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 8B */ Instruction {
            execute: inst_adc_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 8C */ Instruction {
            execute: inst_adc_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 8D */ Instruction {
            execute: inst_adc_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 8E */ Instruction {
            execute: inst_adc_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 8F */ Instruction {
            execute: inst_adc_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 90 */ Instruction {
            execute: inst_sub_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 91 */ Instruction {
            execute: inst_sub_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 92 */ Instruction {
            execute: inst_sub_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 93 */ Instruction {
            execute: inst_sub_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 94 */ Instruction {
            execute: inst_sub_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 95 */ Instruction {
            execute: inst_sub_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 96 */ Instruction {
            execute: inst_sub_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 97 */ Instruction {
            execute: inst_sub_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* 98 */ Instruction {
            execute: inst_sbc_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* 99 */ Instruction {
            execute: inst_sbc_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* 9A */ Instruction {
            execute: inst_sbc_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* 9B */ Instruction {
            execute: inst_sbc_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* 9C */ Instruction {
            execute: inst_sbc_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* 9D */ Instruction {
            execute: inst_sbc_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* 9E */ Instruction {
            execute: inst_sbc_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* 9F */ Instruction {
            execute: inst_sbc_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* A0 */ Instruction {
            execute: inst_and_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* A1 */ Instruction {
            execute: inst_and_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* A2 */ Instruction {
            execute: inst_and_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* A3 */ Instruction {
            execute: inst_and_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* A4 */ Instruction {
            execute: inst_and_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* A5 */ Instruction {
            execute: inst_and_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* A6 */ Instruction {
            execute: inst_and_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* A7 */ Instruction {
            execute: inst_and_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* A8 */ Instruction {
            execute: inst_xor_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* A9 */ Instruction {
            execute: inst_xor_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* AA */ Instruction {
            execute: inst_xor_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* AB */ Instruction {
            execute: inst_xor_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* AC */ Instruction {
            execute: inst_xor_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* AD */ Instruction {
            execute: inst_xor_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* AE */ Instruction {
            execute: inst_xor_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* AF */ Instruction {
            execute: inst_xor_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* B0 */ Instruction {
            execute: inst_or_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* B1 */ Instruction {
            execute: inst_or_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* B2 */ Instruction {
            execute: inst_or_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* B3 */ Instruction {
            execute: inst_or_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* B4 */ Instruction {
            execute: inst_or_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* B5 */ Instruction {
            execute: inst_or_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* B6 */ Instruction {
            execute: inst_or_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* B7 */ Instruction {
            execute: inst_or_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* B8 */ Instruction {
            execute: inst_cp_a_b,
            clock_cycles: 4,
            size: 1,
        },
        /* B9 */ Instruction {
            execute: inst_cp_a_c,
            clock_cycles: 4,
            size: 1,
        },
        /* BA */ Instruction {
            execute: inst_cp_a_d,
            clock_cycles: 4,
            size: 1,
        },
        /* BB */ Instruction {
            execute: inst_cp_a_e,
            clock_cycles: 4,
            size: 1,
        },
        /* BC */ Instruction {
            execute: inst_cp_a_h,
            clock_cycles: 4,
            size: 1,
        },
        /* BD */ Instruction {
            execute: inst_cp_a_l,
            clock_cycles: 4,
            size: 1,
        },
        /* BE */ Instruction {
            execute: inst_cp_a_mem_hl,
            clock_cycles: 7,
            size: 1,
        },
        /* BF */ Instruction {
            execute: inst_cp_a_a,
            clock_cycles: 4,
            size: 1,
        },
        /* C0 */ Instruction {
            execute: inst_ret_nz,
            clock_cycles: 5,
            size: 1,
        },
        /* C1 */ Instruction {
            execute: inst_pop_bc,
            clock_cycles: 10,
            size: 1,
        },
        /* C2 */ Instruction {
            execute: inst_jp_nz_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* C3 */ Instruction {
            execute: inst_jp_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* C4 */ Instruction {
            execute: inst_call_nz_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* C5 */ Instruction {
            execute: inst_push_bc,
            clock_cycles: 11,
            size: 1,
        },
        /* C6 */ Instruction {
            execute: inst_add_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* C7 */ Instruction {
            execute: inst_rst_00h,
            clock_cycles: 11,
            size: 1,
        },
        /* C8 */ Instruction {
            execute: inst_ret_z,
            clock_cycles: 5,
            size: 1,
        },
        /* C9 */ Instruction {
            execute: inst_ret,
            clock_cycles: 10,
            size: 1,
        },
        /* CA */ Instruction {
            execute: inst_jp_z_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* CB */ Instruction {  // This is the bit manip. instruction prefix.
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CC */ Instruction {
            execute: inst_call_z_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* CD */ Instruction {
            execute: inst_call_im16,
            clock_cycles: 17,
            size: 3,
        },
        /* CE */ Instruction {
            execute: inst_adc_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* CF */ Instruction {
            execute: inst_rst_08h,
            clock_cycles: 11,
            size: 1,
        },
        /* D0 */ Instruction {
            execute: inst_ret_nc,
            clock_cycles: 5,
            size: 1,
        },
        /* D1 */ Instruction {
            execute: inst_pop_de,
            clock_cycles: 10,
            size: 1,
        },
        /* D2 */ Instruction {
            execute: inst_jp_nc_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* D3 */ Instruction {
            execute: inst_out_im8_a,
            clock_cycles: 11,
            size: 2,
        },
        /* D4 */ Instruction {
            execute: inst_call_nc_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* D5 */ Instruction {
            execute: inst_push_de,
            clock_cycles: 11,
            size: 1,
        },
        /* D6 */ Instruction {
            execute: inst_sub_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* D7 */ Instruction {
            execute: inst_rst_10h,
            clock_cycles: 11,
            size: 1,
        },
        /* D8 */ Instruction {
            execute: inst_ret_c,
            clock_cycles: 5,
            size: 1,
        },
        /* D9 */ Instruction {
            execute: inst_exx,
            clock_cycles: 4,
            size: 1,
        },
        /* DA */ Instruction {
            execute: inst_jp_c_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* DB */ Instruction {
            execute: inst_in_a_im8,
            clock_cycles: 11,
            size: 2,
        },
        /* DC */ Instruction {
            execute: inst_call_c_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* DD */ Instruction {  // This is the IX instruction prefix.
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DE */ Instruction {
            execute: inst_sbc_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* DF */ Instruction {
            execute: inst_rst_18h,
            clock_cycles: 11,
            size: 1,
        },
        /* E0 */ Instruction {
            execute: inst_ret_po,
            clock_cycles: 5,
            size: 1,
        },
        /* E1 */ Instruction {
            execute: inst_pop_hl,
            clock_cycles: 10,
            size: 1,
        },
        /* E2 */ Instruction {
            execute: inst_jp_po_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* E3 */ Instruction {
            execute: inst_ex_mem_sp_hl,
            clock_cycles: 19,
            size: 1,
        },
        /* E4 */ Instruction {
            execute: inst_call_po_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* E5 */ Instruction {
            execute: inst_push_hl,
            clock_cycles: 11,
            size: 1,
        },
        /* E6 */ Instruction {
            execute: inst_and_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* E7 */ Instruction {
            execute: inst_rst_20h,
            clock_cycles: 11,
            size: 1,
        },
        /* E8 */ Instruction {
            execute: inst_ret_pe,
            clock_cycles: 5,
            size: 1,
        },
        /* E9 */ Instruction {
            execute: inst_jp_hl,
            clock_cycles: 4,
            size: 1,
        },
        /* EA */ Instruction {
            execute: inst_jp_pe_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* EB */ Instruction {
            execute: inst_ex_de_hl,
            clock_cycles: 4,
            size: 1,
        },
        /* EC */ Instruction {
            execute: inst_call_pe_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* ED */ Instruction {  // This is the extended instruction prefix.
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EE */ Instruction {
            execute: inst_xor_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* EF */ Instruction {
            execute: inst_rst_28h,
            clock_cycles: 11,
            size: 1,
        },
        /* F0 */ Instruction {
            execute: inst_ret_p,
            clock_cycles: 5,
            size: 1,
        },
        /* F1 */ Instruction {
            execute: inst_pop_af,
            clock_cycles: 10,
            size: 1,
        },
        /* F2 */ Instruction {
            execute: inst_jp_p_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* F3 */ Instruction {
            execute: inst_di,
            clock_cycles: 4,
            size: 1,
        },
        /* F4 */ Instruction {
            execute: inst_call_p_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* F5 */ Instruction {
            execute: inst_push_af,
            clock_cycles: 11,
            size: 1,
        },
        /* F6 */ Instruction {
            execute: inst_or_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* F7 */ Instruction {
            execute: inst_rst_30h,
            clock_cycles: 11,
            size: 1,
        },
        /* F8 */ Instruction {
            execute: inst_ret_m,
            clock_cycles: 5,
            size: 1,
        },
        /* F9 */ Instruction {
            execute: inst_ld_sp_hl,
            clock_cycles: 6,
            size: 1,
        },
        /* FA */ Instruction {
            execute: inst_jp_m_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* FB */ Instruction {
            execute: inst_ei,
            clock_cycles: 4,
            size: 1,
        },
        /* FC */ Instruction {
            execute: inst_call_m_im16,
            clock_cycles: 10,
            size: 3,
        },
        /* FD */ Instruction {  // This is the IY instruction prefix.
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FE */ Instruction {
            execute: inst_cp_a_im8,
            clock_cycles: 7,
            size: 2,
        },
        /* FF */ Instruction {
            execute: inst_rst_38h,
            clock_cycles: 11,
            size: 1,
        },
    ],
    // ED-Prefixed (Extended) instructions:
    extended: [
        /* 40 */ Instruction {
            execute: inst_in_b_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 41 */ Instruction {
            execute: inst_out_mem_bc_b,
            clock_cycles: 12,
            size: 2,
        },
        /* 42 */ Instruction {
            execute: inst_sbc_hl_bc,
            clock_cycles: 15,
            size: 2,
        },
        /* 43 */ Instruction {
            execute: inst_ld_mem_im16_bc,
            clock_cycles: 20,
            size: 4,
        },
        /* 44 */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 45 */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 46 */ Instruction {
            execute: inst_im0,
            clock_cycles: 8,
            size: 2,
        },
        /* 47 */ Instruction {
            execute: inst_ld_i_a,
            clock_cycles: 9,
            size: 2,
        },
        /* 48 */ Instruction {
            execute: inst_in_c_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 49 */ Instruction {
            execute: inst_out_mem_bc_c,
            clock_cycles: 12,
            size: 2,
        },
        /* 4A */ Instruction {
            execute: inst_adc_hl_bc,
            clock_cycles: 15,
            size: 2,
        },
        /* 4B */ Instruction {
            execute: inst_ld_bc_mem_im16,
           clock_cycles: 20,
           size: 4,
        },
        /* 4C */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 4D */ Instruction {
            execute: inst_reti,
            clock_cycles: 14,
            size: 2,
        },
        /* 4E */ Instruction {
            execute: inst_im_0_slash_1,
            clock_cycles: 8,
            size: 2,
        },
        /* 4F */ Instruction {
            execute: inst_ld_r_a,
            clock_cycles: 9,
            size: 2,
        },
        /* 50 */ Instruction {
            execute: inst_in_d_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 51 */ Instruction {
            execute: inst_out_mem_bc_d,
            clock_cycles: 12,
            size: 2,
        },
        /* 52 */ Instruction {
            execute: inst_sbc_hl_de,
            clock_cycles: 15,
            size: 2,
        },
        /* 53 */ Instruction {
            execute: inst_ld_mem_im16_de,
            clock_cycles: 20,
            size: 4,
        },
        /* 54 */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 55 */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 56 */ Instruction {
            execute: inst_im1,
            clock_cycles: 8,
            size: 2,
        },
        /* 57 */ Instruction {
            execute: inst_ld_a_i,
            clock_cycles: 9,
            size: 2,
        },
        /* 58 */ Instruction {
            execute: inst_in_e_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 59 */ Instruction {
            execute: inst_out_mem_bc_e,
            clock_cycles: 12,
            size: 2,
        },
        /* 5A */ Instruction {
            execute: inst_adc_hl_de,
            clock_cycles: 15,
            size: 2,
        },
        /* 5B */ Instruction {
            execute: inst_ld_de_mem_im16,
            clock_cycles: 20,
            size: 4,
        },
        /* 5C */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 5D */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 5E */ Instruction {
            execute: inst_im_2,
            clock_cycles: 8,
            size: 2,
        },
        /* 5F */ Instruction {
            execute: inst_ld_a_r,
            clock_cycles: 9,
            size: 2,
        },
        /* 60 */ Instruction {
            execute: inst_in_h_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 61 */ Instruction {
            execute: inst_out_mem_bc_h,
            clock_cycles: 12,
            size: 2,
        },
        /* 62 */ Instruction {
            execute: inst_sbc_hl_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 63 */ Instruction {
            execute: inst_ld_mem_im16_hl_2,
            clock_cycles: 20,
            size: 4,
        },
        /* 64 */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 65 */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 66 */ Instruction {
            execute: inst_im0,
            clock_cycles: 8,
            size: 2,
        },
        /* 67 */ Instruction {
            execute: inst_rrd,
            clock_cycles: 18,
            size: 2,
        },
        /* 68 */ Instruction {
            execute: inst_in_l_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 69 */ Instruction {
            execute: inst_out_mem_bc_l,
            clock_cycles: 12,
            size: 2,
        },
        /* 6A */ Instruction {
            execute: inst_adc_hl_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 6B */ Instruction {
            execute: inst_ld_hl_mem_im16_2,
            clock_cycles: 20,
            size: 4,
        },
        /* 6C */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 6D */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 6E */ Instruction {
            execute: inst_im_0_slash_1,
            clock_cycles: 8,
            size: 2,
        },
        /* 6F */ Instruction {
            execute: inst_rld,
            clock_cycles: 18,
            size: 2,
        },
        /* 70 */ Instruction {
            execute: inst_in_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 71 */ Instruction {
            execute: inst_out_mem_bc_0,
            clock_cycles: 12,
            size: 2,
        },
        /* 72 */ Instruction {
            execute: inst_sbc_hl_sp,
            clock_cycles: 15,
            size: 2,
        },
        /* 73 */ Instruction {
            execute: inst_ld_mem_im16_sp,
            clock_cycles: 20,
            size: 4,
        },
        /* 74 */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 75 */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 76 */ Instruction {
            execute: inst_im1,
            clock_cycles: 8,
            size: 2,
        },
        /* 77 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* 78 */ Instruction {
            execute: inst_in_a_mem_bc,
            clock_cycles: 12,
            size: 2,
        },
        /* 79 */ Instruction {
            execute: inst_out_mem_bc_a,
            clock_cycles: 12,
            size: 2,
        },
        /* 7A */ Instruction {
            execute: inst_adc_hl_sp,
            clock_cycles: 15,
            size: 2,
        },
        /* 7B */ Instruction {
            execute: inst_ld_sp_mem_im16,
            clock_cycles: 20,
            size: 4,
        },
        /* 7C */ Instruction {
            execute: inst_neg,
            clock_cycles: 8,
            size: 2,
        },
        /* 7D */ Instruction {
            execute: inst_retn,
            clock_cycles: 14,
            size: 2,
        },
        /* 7E */ Instruction {
            execute: inst_im_2,
            clock_cycles: 8,
            size: 2,
        },
        /* 7F */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* A0 */ Instruction {
            execute: inst_ldi,
            clock_cycles: 16,
            size: 2,
        },
        /* A1 */ Instruction {
            execute: inst_cpi,
            clock_cycles: 16,
            size: 2,
        },
        /* A2 */ Instruction {
            execute: inst_ini,
            clock_cycles: 16,
            size: 2,
        },
        /* A3 */ Instruction {
            execute: inst_outi,
            clock_cycles: 16,
            size: 2,
        },
        /* A4 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* A5 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* A6 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* A7 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* A8 */ Instruction {
            execute: inst_ldd,
            clock_cycles: 16,
            size: 2,
        },
        /* A9 */ Instruction {
            execute: inst_cpd,
            clock_cycles: 16,
            size: 2,
        },
        /* AA */ Instruction {
            execute: inst_ind,
            clock_cycles: 16,
            size: 2,
        },
        /* AB */ Instruction {
            execute: inst_outd,
            clock_cycles: 16,
            size: 2,
        },
        /* AC */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* AD */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* AE */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* AF */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* B0 */ Instruction {
            execute: inst_ldir,
            clock_cycles: 16,
            size: 2,
        },
        /* B1 */ Instruction {
            execute: inst_cpir,
            clock_cycles: 16,
            size: 2,
        },
        /* B2 */ Instruction {
            execute: inst_inir,
            clock_cycles: 16,
            size: 2,
        },
        /* B3 */ Instruction {
            execute: inst_outir,
            clock_cycles: 16,
            size: 2,
        },
        /* B4 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* B5 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* B6 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* B7 */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* B8 */ Instruction {
            execute: inst_lddr,
            clock_cycles: 16,
            size: 2,
        },
        /* B9 */ Instruction {
            execute: inst_cpdr,
            clock_cycles: 16,
            size: 2,
        },
        /* BA */ Instruction {
            execute: inst_indr,
            clock_cycles: 16,
            size: 2,
        },
        /* BB */ Instruction {
            execute: inst_outdr,
            clock_cycles: 16,
            size: 2,
        },
        /* BC */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* BD */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* BE */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* BF */ Instruction {
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
    ],
    // CB-Prefixed (bit manipulation) instructions:
    bit: [
        /* 00 */ Instruction {
            execute: inst_rlc_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 01 */ Instruction {
            execute: inst_rlc_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 02 */ Instruction {
            execute: inst_rlc_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 03 */ Instruction {
            execute: inst_rlc_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 04 */ Instruction {
            execute: inst_rlc_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 05 */ Instruction {
            execute: inst_rlc_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 06 */ Instruction {
            execute: inst_rlc_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 07 */ Instruction {
            execute: inst_rlc_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 08 */ Instruction {
            execute: inst_rrc_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 09 */ Instruction {
            execute: inst_rrc_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 0A */ Instruction {
            execute: inst_rrc_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 0B */ Instruction {
            execute: inst_rrc_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 0C */ Instruction {
            execute: inst_rrc_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 0D */ Instruction {
            execute: inst_rrc_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 0E */ Instruction {
            execute: inst_rrc_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 0F */ Instruction {
            execute: inst_rrc_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 10 */ Instruction {
            execute: inst_rl_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 11 */ Instruction {
            execute: inst_rl_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 12 */ Instruction {
            execute: inst_rl_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 13 */ Instruction {
            execute: inst_rl_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 14 */ Instruction {
            execute: inst_rl_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 15 */ Instruction {
            execute: inst_rl_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 16 */ Instruction {
            execute: inst_rl_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 17 */ Instruction {
            execute: inst_rl_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 18 */ Instruction {
            execute: inst_rr_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 19 */ Instruction {
            execute: inst_rr_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 1A */ Instruction {
            execute: inst_rr_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 1B */ Instruction {
            execute: inst_rr_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 1C */ Instruction {
            execute: inst_rr_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 1D */ Instruction {
            execute: inst_rr_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 1E */ Instruction {
            execute: inst_rr_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 1F */ Instruction {
            execute: inst_rr_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 20 */ Instruction {
            execute: inst_sla_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 21 */ Instruction {
            execute: inst_sla_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 22 */ Instruction {
            execute: inst_sla_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 23 */ Instruction {
            execute: inst_sla_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 24 */ Instruction {
            execute: inst_sla_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 25 */ Instruction {
            execute: inst_sla_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 26 */ Instruction {
            execute: inst_sla_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 27 */ Instruction {
            execute: inst_sla_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 28 */ Instruction {
            execute: inst_sra_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 29 */ Instruction {
            execute: inst_sra_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 2A */ Instruction {
            execute: inst_sra_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 2B */ Instruction {
            execute: inst_sra_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 2C */ Instruction {
            execute: inst_sra_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 2D */ Instruction {
            execute: inst_sra_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 2E */ Instruction {
            execute: inst_sra_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 2F */ Instruction {
            execute: inst_sra_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 30 */ Instruction {
            execute: inst_sll_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 31 */ Instruction {
            execute: inst_sll_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 32 */ Instruction {
            execute: inst_sll_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 33 */ Instruction {
            execute: inst_sll_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 34 */ Instruction {
            execute: inst_sll_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 35 */ Instruction {
            execute: inst_sll_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 36 */ Instruction {
            execute: inst_sll_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 37 */ Instruction {
            execute: inst_sll_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 38 */ Instruction {
            execute: inst_srl_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 39 */ Instruction {
            execute: inst_srl_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 3A */ Instruction {
            execute: inst_srl_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 3B */ Instruction {
            execute: inst_srl_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 3C */ Instruction {
            execute: inst_srl_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 3D */ Instruction {
            execute: inst_srl_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 3E */ Instruction {
            execute: inst_srl_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 3F */ Instruction {
            execute: inst_srl_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 40 */ Instruction {
            execute: inst_bit_0_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 41 */ Instruction {
            execute: inst_bit_0_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 42 */ Instruction {
            execute: inst_bit_0_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 43 */ Instruction {
            execute: inst_bit_0_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 44 */ Instruction {
            execute: inst_bit_0_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 45 */ Instruction {
            execute: inst_bit_0_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 46 */ Instruction {
            execute: inst_bit_0_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 47 */ Instruction {
            execute: inst_bit_0_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 48 */ Instruction {
            execute: inst_bit_1_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 49 */ Instruction {
            execute: inst_bit_1_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 4A */ Instruction {
            execute: inst_bit_1_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 4B */ Instruction {
            execute: inst_bit_1_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 4C */ Instruction {
            execute: inst_bit_1_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 4D */ Instruction {
            execute: inst_bit_1_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 4E */ Instruction {
            execute: inst_bit_1_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 4F */ Instruction {
            execute: inst_bit_1_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 50 */ Instruction {
            execute: inst_bit_2_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 51 */ Instruction {
            execute: inst_bit_2_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 52 */ Instruction {
            execute: inst_bit_2_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 53 */ Instruction {
            execute: inst_bit_2_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 54 */ Instruction {
            execute: inst_bit_2_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 55 */ Instruction {
            execute: inst_bit_2_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 56 */ Instruction {
            execute: inst_bit_2_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 57 */ Instruction {
            execute: inst_bit_2_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 58 */ Instruction {
            execute: inst_bit_3_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 59 */ Instruction {
            execute: inst_bit_3_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 5A */ Instruction {
            execute: inst_bit_3_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 5B */ Instruction {
            execute: inst_bit_3_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 5C */ Instruction {
            execute: inst_bit_3_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 5D */ Instruction {
            execute: inst_bit_3_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 5E */ Instruction {
            execute: inst_bit_3_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 5F */ Instruction {
            execute: inst_bit_3_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 60 */ Instruction {
            execute: inst_bit_4_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 61 */ Instruction {
            execute: inst_bit_4_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 62 */ Instruction {
            execute: inst_bit_4_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 63 */ Instruction {
            execute: inst_bit_4_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 64 */ Instruction {
            execute: inst_bit_4_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 65 */ Instruction {
            execute: inst_bit_4_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 66 */ Instruction {
            execute: inst_bit_4_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 67 */ Instruction {
            execute: inst_bit_4_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 68 */ Instruction {
            execute: inst_bit_5_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 69 */ Instruction {
            execute: inst_bit_5_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 6A */ Instruction {
            execute: inst_bit_5_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 6B */ Instruction {
            execute: inst_bit_5_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 6C */ Instruction {
            execute: inst_bit_5_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 6D */ Instruction {
            execute: inst_bit_5_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 6E */ Instruction {
            execute: inst_bit_5_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 6F */ Instruction {
            execute: inst_bit_5_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 70 */ Instruction {
            execute: inst_bit_6_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 71 */ Instruction {
            execute: inst_bit_6_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 72 */ Instruction {
            execute: inst_bit_6_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 73 */ Instruction {
            execute: inst_bit_6_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 74 */ Instruction {
            execute: inst_bit_6_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 75 */ Instruction {
            execute: inst_bit_6_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 76 */ Instruction {
            execute: inst_bit_6_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 77 */ Instruction {
            execute: inst_bit_6_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 78 */ Instruction {
            execute: inst_bit_7_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 79 */ Instruction {
            execute: inst_bit_7_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 7A */ Instruction {
            execute: inst_bit_7_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 7B */ Instruction {
            execute: inst_bit_7_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 7C */ Instruction {
            execute: inst_bit_7_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 7D */ Instruction {
            execute: inst_bit_7_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 7E */ Instruction {
            execute: inst_bit_7_mem_hl,
            clock_cycles: 12,
            size: 2,
        },
        /* 7F */ Instruction {
            execute: inst_bit_7_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 80 */ Instruction {
            execute: inst_res_0_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 81 */ Instruction {
            execute: inst_res_0_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 82 */ Instruction {
            execute: inst_res_0_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 83 */ Instruction {
            execute: inst_res_0_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 84 */ Instruction {
            execute: inst_res_0_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 85 */ Instruction {
            execute: inst_res_0_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 86 */ Instruction {
            execute: inst_res_0_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 87 */ Instruction {
            execute: inst_res_0_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 88 */ Instruction {
            execute: inst_res_1_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 89 */ Instruction {
            execute: inst_res_1_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 8A */ Instruction {
            execute: inst_res_1_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 8B */ Instruction {
            execute: inst_res_1_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 8C */ Instruction {
            execute: inst_res_1_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 8D */ Instruction {
            execute: inst_res_1_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 8E */ Instruction {
            execute: inst_res_1_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 8F */ Instruction {
            execute: inst_res_1_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 90 */ Instruction {
            execute: inst_res_2_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 91 */ Instruction {
            execute: inst_res_2_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 92 */ Instruction {
            execute: inst_res_2_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 93 */ Instruction {
            execute: inst_res_2_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 94 */ Instruction {
            execute: inst_res_2_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 95 */ Instruction {
            execute: inst_res_2_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 96 */ Instruction {
            execute: inst_res_2_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 97 */ Instruction {
            execute: inst_res_2_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 98 */ Instruction {
            execute: inst_res_3_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 99 */ Instruction {
            execute: inst_res_3_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 9A */ Instruction {
            execute: inst_res_3_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 9B */ Instruction {
            execute: inst_res_3_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 9C */ Instruction {
            execute: inst_res_3_h,
            clock_cycles: 8,
            size: 2,
        },
        /* 9D */ Instruction {
            execute: inst_res_3_l,
            clock_cycles: 8,
            size: 2,
        },
        /* 9E */ Instruction {
            execute: inst_res_3_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* 9F */ Instruction {
            execute: inst_res_3_a,
            clock_cycles: 8,
            size: 2,
        },
        /* A0 */ Instruction {
            execute: inst_res_4_b,
            clock_cycles: 8,
            size: 2,
        },
        /* A1 */ Instruction {
            execute: inst_res_4_c,
            clock_cycles: 8,
            size: 2,
        },
        /* A2 */ Instruction {
            execute: inst_res_4_d,
            clock_cycles: 8,
            size: 2,
        },
        /* A3 */ Instruction {
            execute: inst_res_4_e,
            clock_cycles: 8,
            size: 2,
        },
        /* A4 */ Instruction {
            execute: inst_res_4_h,
            clock_cycles: 8,
            size: 2,
        },
        /* A5 */ Instruction {
            execute: inst_res_4_l,
            clock_cycles: 8,
            size: 2,
        },
        /* A6 */ Instruction {
            execute: inst_res_4_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* A7 */ Instruction {
            execute: inst_res_4_a,
            clock_cycles: 8,
            size: 2,
        },
        /* A8 */ Instruction {
            execute: inst_res_5_b,
            clock_cycles: 8,
            size: 2,
        },
        /* A9 */ Instruction {
            execute: inst_res_5_c,
            clock_cycles: 8,
            size: 2,
        },
        /* AA */ Instruction {
            execute: inst_res_5_d,
            clock_cycles: 8,
            size: 2,
        },
        /* AB */ Instruction {
            execute: inst_res_5_e,
            clock_cycles: 8,
            size: 2,
        },
        /* AC */ Instruction {
            execute: inst_res_5_h,
            clock_cycles: 8,
            size: 2,
        },
        /* AD */ Instruction {
            execute: inst_res_5_l,
            clock_cycles: 8,
            size: 2,
        },
        /* AE */ Instruction {
            execute: inst_res_5_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* AF */ Instruction {
            execute: inst_res_5_a,
            clock_cycles: 8,
            size: 2,
        },
        /* B0 */ Instruction {
            execute: inst_res_6_b,
            clock_cycles: 8,
            size: 2,
        },
        /* B1 */ Instruction {
            execute: inst_res_6_c,
            clock_cycles: 8,
            size: 2,
        },
        /* B2 */ Instruction {
            execute: inst_res_6_d,
            clock_cycles: 8,
            size: 2,
        },
        /* B3 */ Instruction {
            execute: inst_res_6_e,
            clock_cycles: 8,
            size: 2,
        },
        /* B4 */ Instruction {
            execute: inst_res_6_h,
            clock_cycles: 8,
            size: 2,
        },
        /* B5 */ Instruction {
            execute: inst_res_6_l,
            clock_cycles: 8,
            size: 2,
        },
        /* B6 */ Instruction {
            execute: inst_res_6_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* B7 */ Instruction {
            execute: inst_res_6_a,
            clock_cycles: 8,
            size: 2,
        },
        /* B8 */ Instruction {
            execute: inst_res_7_b,
            clock_cycles: 8,
            size: 2,
        },
        /* B9 */ Instruction {
            execute: inst_res_7_c,
            clock_cycles: 8,
            size: 2,
        },
        /* BA */ Instruction {
            execute: inst_res_7_d,
            clock_cycles: 8,
            size: 2,
        },
        /* BB */ Instruction {
            execute: inst_res_7_e,
            clock_cycles: 8,
            size: 2,
        },
        /* BC */ Instruction {
            execute: inst_res_7_h,
            clock_cycles: 8,
            size: 2,
        },
        /* BD */ Instruction {
            execute: inst_res_7_l,
            clock_cycles: 8,
            size: 2,
        },
        /* BE */ Instruction {
            execute: inst_res_7_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* BF */ Instruction {
            execute: inst_res_7_a,
            clock_cycles: 8,
            size: 2,
        },
        /* C0 */ Instruction {
            execute: inst_set_0_b,
            clock_cycles: 8,
            size: 2,
        },
        /* C1 */ Instruction {
            execute: inst_set_0_c,
            clock_cycles: 8,
            size: 2,
        },
        /* C2 */ Instruction {
            execute: inst_set_0_d,
            clock_cycles: 8,
            size: 2,
        },
        /* C3 */ Instruction {
            execute: inst_set_0_e,
            clock_cycles: 8,
            size: 2,
        },
        /* C4 */ Instruction {
            execute: inst_set_0_h,
            clock_cycles: 8,
            size: 2,
        },
        /* C5 */ Instruction {
            execute: inst_set_0_l,
            clock_cycles: 8,
            size: 2,
        },
        /* C6 */ Instruction {
            execute: inst_set_0_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* C7 */ Instruction {
            execute: inst_set_0_a,
            clock_cycles: 8,
            size: 2,
        },
        /* C8 */ Instruction {
            execute: inst_set_1_b,
            clock_cycles: 8,
            size: 2,
        },
        /* C9 */ Instruction {
            execute: inst_set_1_c,
            clock_cycles: 8,
            size: 2,
        },
        /* CA */ Instruction {
            execute: inst_set_1_d,
            clock_cycles: 8,
            size: 2,
        },
        /* CB */ Instruction {
            execute: inst_set_1_e,
            clock_cycles: 8,
            size: 2,
        },
        /* CC */ Instruction {
            execute: inst_set_1_h,
            clock_cycles: 8,
            size: 2,
        },
        /* CD */ Instruction {
            execute: inst_set_1_l,
            clock_cycles: 8,
            size: 2,
        },
        /* CE */ Instruction {
            execute: inst_set_1_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* CF */ Instruction {
            execute: inst_set_1_a,
            clock_cycles: 8,
            size: 2,
        },
        /* D0 */ Instruction {
            execute: inst_set_2_b,
            clock_cycles: 8,
            size: 2,
        },
        /* D1 */ Instruction {
            execute: inst_set_2_c,
            clock_cycles: 8,
            size: 2,
        },
        /* D2 */ Instruction {
            execute: inst_set_2_d,
            clock_cycles: 8,
            size: 2,
        },
        /* D3 */ Instruction {
            execute: inst_set_2_e,
            clock_cycles: 8,
            size: 2,
        },
        /* D4 */ Instruction {
            execute: inst_set_2_h,
            clock_cycles: 8,
            size: 2,
        },
        /* D5 */ Instruction {
            execute: inst_set_2_l,
            clock_cycles: 8,
            size: 2,
        },
        /* D6 */ Instruction {
            execute: inst_set_2_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* D7 */ Instruction {
            execute: inst_set_2_a,
            clock_cycles: 8,
            size: 2,
        },
        /* D8 */ Instruction {
            execute: inst_set_3_b,
            clock_cycles: 8,
            size: 2,
        },
        /* D9 */ Instruction {
            execute: inst_set_3_c,
            clock_cycles: 8,
            size: 2,
        },
        /* DA */ Instruction {
            execute: inst_set_3_d,
            clock_cycles: 8,
            size: 2,
        },
        /* DB */ Instruction {
            execute: inst_set_3_e,
            clock_cycles: 8,
            size: 2,
        },
        /* DC */ Instruction {
            execute: inst_set_3_h,
            clock_cycles: 8,
            size: 2,
        },
        /* DD */ Instruction {
            execute: inst_set_3_l,
            clock_cycles: 8,
            size: 2,
        },
        /* DE */ Instruction {
            execute: inst_set_3_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* DF */ Instruction {
            execute: inst_set_3_a,
            clock_cycles: 8,
            size: 2,
        },
        /* E0 */ Instruction {
            execute: inst_set_4_b,
            clock_cycles: 8,
            size: 2,
        },
        /* E1 */ Instruction {
            execute: inst_set_4_c,
            clock_cycles: 8,
            size: 2,
        },
        /* E2 */ Instruction {
            execute: inst_set_4_d,
            clock_cycles: 8,
            size: 2,
        },
        /* E3 */ Instruction {
            execute: inst_set_4_e,
            clock_cycles: 8,
            size: 2,
        },
        /* E4 */ Instruction {
            execute: inst_set_4_h,
            clock_cycles: 8,
            size: 2,
        },
        /* E5 */ Instruction {
            execute: inst_set_4_l,
            clock_cycles: 8,
            size: 2,
        },
        /* E6 */ Instruction {
            execute: inst_set_4_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* E7 */ Instruction {
            execute: inst_set_4_a,
            clock_cycles: 8,
            size: 2,
        },
        /* E8 */ Instruction {
            execute: inst_set_5_b,
            clock_cycles: 8,
            size: 2,
        },
        /* E9 */ Instruction {
            execute: inst_set_5_c,
            clock_cycles: 8,
            size: 2,
        },
        /* EA */ Instruction {
            execute: inst_set_5_d,
            clock_cycles: 8,
            size: 2,
        },
        /* EB */ Instruction {
            execute: inst_set_5_e,
            clock_cycles: 8,
            size: 2,
        },
        /* EC */ Instruction {
            execute: inst_set_5_h,
            clock_cycles: 8,
            size: 2,
        },
        /* ED */ Instruction {
            execute: inst_set_5_l,
            clock_cycles: 8,
            size: 2,
        },
        /* EE */ Instruction {
            execute: inst_set_5_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* EF */ Instruction {
            execute: inst_set_5_a,
            clock_cycles: 8,
            size: 2,
        },
        /* F0 */ Instruction {
            execute: inst_set_6_b,
            clock_cycles: 8,
            size: 2,
        },
        /* F1 */ Instruction {
            execute: inst_set_6_c,
            clock_cycles: 8,
            size: 2,
        },
        /* F2 */ Instruction {
            execute: inst_set_6_d,
            clock_cycles: 8,
            size: 2,
        },
        /* F3 */ Instruction {
            execute: inst_set_6_e,
            clock_cycles: 8,
            size: 2,
        },
        /* F4 */ Instruction {
            execute: inst_set_6_h,
            clock_cycles: 8,
            size: 2,
        },
        /* F5 */ Instruction {
            execute: inst_set_6_l,
            clock_cycles: 8,
            size: 2,
        },
        /* F6 */ Instruction {
            execute: inst_set_6_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* F7 */ Instruction {
            execute: inst_set_6_a,
            clock_cycles: 8,
            size: 2,
        },
        /* F8 */ Instruction {
            execute: inst_set_7_b,
            clock_cycles: 8,
            size: 2,
        },
        /* F9 */ Instruction {
            execute: inst_set_7_c,
            clock_cycles: 8,
            size: 2,
        },
        /* FA */ Instruction {
            execute: inst_set_7_d,
            clock_cycles: 8,
            size: 2,
        },
        /* FB */ Instruction {
            execute: inst_set_7_e,
            clock_cycles: 8,
            size: 2,
        },
        /* FC */ Instruction {
            execute: inst_set_7_h,
            clock_cycles: 8,
            size: 2,
        },
        /* FD */ Instruction {
            execute: inst_set_7_l,
            clock_cycles: 8,
            size: 2,
        },
        /* FE */ Instruction {
            execute: inst_set_7_mem_hl,
            clock_cycles: 15,
            size: 2,
        },
        /* FF */ Instruction {
            execute: inst_set_7_a,
            clock_cycles: 8,
            size: 2,
        },
    ],
    // DD-Prefixed (IX) instructions:

    // DD acts as a modifier prefix, therefore if it doesn't affect an
    // instruction's operation, it is interpreted as a NOP.
    ix: [
        /* 00 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 01 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 02 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 03 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 04 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 05 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 06 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 07 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 08 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 09 */ Instruction {
            execute: inst_add_ix_bc,
            clock_cycles: 15,
            size: 2,
        },
        /* 0A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 10 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 11 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 12 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 13 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 14 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 15 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 16 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 17 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 18 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 19 */ Instruction {
            execute: inst_add_ix_de,
            clock_cycles: 15,
            size: 2,
        },
        /* 1A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 20 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 21 */ Instruction {
            execute: inst_ld_ix_im16,
            clock_cycles: 14,
            size: 4,
        },
        /* 22 */ Instruction {
            execute: inst_ld_mem_im16_ix,
            clock_cycles: 20,
            size: 4,
        },
        /* 23 */ Instruction {
            execute: inst_inc_ix,
            clock_cycles: 10,
            size: 2,
        },
        /* 24 */ Instruction {
            execute: inst_inc_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 25 */ Instruction {
            execute: inst_dec_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 26 */ Instruction {
            execute: inst_ld_ixh_im8,
            clock_cycles: 11,
            size: 3,
        },
        /* 27 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 28 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 29 */ Instruction {
            execute: inst_add_ix_ix,
            clock_cycles: 15,
            size: 2,
        },
        /* 2A */ Instruction {
            execute: inst_ld_ix_mem_im16,
            clock_cycles: 20,
            size: 4,
        },
        /* 2B */ Instruction {
            execute: inst_dec_ix,
            clock_cycles: 10,
            size: 2,
        },
        /* 2C */ Instruction {
            execute: inst_inc_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 2D */ Instruction {
            execute: inst_dec_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 2E */ Instruction {
            execute: inst_ld_ixl_im8,
            clock_cycles: 11,
            size: 3,
        },
        /* 2F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 30 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 31 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 32 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 33 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 34 */ Instruction {
            execute: inst_inc_mem_ix_im8,
            clock_cycles: 23,
            size: 3,
        },
        /* 35 */ Instruction {
            execute: inst_dec_mem_ix_im8,
            clock_cycles: 23,
            size: 3,
        },
        /* 36 */ Instruction {
            execute: inst_ld_mem_ix_im8_im8,
            clock_cycles: 19,
            size: 4,
        },
        /* 37 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 38 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 39 */ Instruction {
            execute: inst_add_ix_sp,
            clock_cycles: 15,
            size: 2,
        },
        /* 3A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 40 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 41 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 42 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 43 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 44 */ Instruction {
            execute: inst_ld_b_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 45 */ Instruction {
            execute: inst_ld_b_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 46 */ Instruction {
            execute: inst_ld_b_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 47 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 48 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 49 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4C */ Instruction {
            execute: inst_ld_c_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 4D */ Instruction {
            execute: inst_ld_c_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 4E */ Instruction {
            execute: inst_ld_c_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 4F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 50 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 51 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 52 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 53 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 54 */ Instruction {
            execute: inst_ld_d_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 55 */ Instruction {
            execute: inst_ld_d_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 56 */ Instruction {
            execute: inst_ld_d_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 57 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 58 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 59 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5C */ Instruction {
            execute: inst_ld_e_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 5D */ Instruction {
            execute: inst_ld_e_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 5E */ Instruction {
            execute: inst_ld_e_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 5F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 60 */ Instruction {
            execute: inst_ld_ixh_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 61 */ Instruction {
            execute: inst_ld_ixh_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 62 */ Instruction {
            execute: inst_ld_ixh_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 63 */ Instruction {
            execute: inst_ld_ixh_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 64 */ Instruction {
            execute: inst_ld_ixh_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 65 */ Instruction {
            execute: inst_ld_ixh_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 66 */ Instruction {
            execute: inst_ld_h_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 67 */ Instruction {
            execute: inst_ld_ixh_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 68 */ Instruction {
            execute: inst_ld_ixl_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 69 */ Instruction {
            execute: inst_ld_ixl_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 6A */ Instruction {
            execute: inst_ld_ixl_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 6B */ Instruction {
            execute: inst_ld_ixl_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 6C */ Instruction {
            execute: inst_ld_ixl_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 6D */ Instruction {
            execute: inst_ld_ixl_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 6E */ Instruction {
            execute: inst_ld_l_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 6F */ Instruction {
            execute: inst_ld_ixl_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 70 */ Instruction {
            execute: inst_ld_mem_ix_im8_b,
            clock_cycles: 19,
            size: 3,
        },
        /* 71 */ Instruction {
            execute: inst_ld_mem_ix_im8_c,
            clock_cycles: 19,
            size: 3,
        },
        /* 72 */ Instruction {
            execute: inst_ld_mem_ix_im8_d,
            clock_cycles: 19,
            size: 3,
        },
        /* 73 */ Instruction {
            execute: inst_ld_mem_ix_im8_e,
            clock_cycles: 19,
            size: 3,
        },
        /* 74 */ Instruction {
            execute: inst_ld_mem_ix_im8_h,
            clock_cycles: 19,
            size: 3,
        },
        /* 75 */ Instruction {
            execute: inst_ld_mem_ix_im8_l,
            clock_cycles: 19,
            size: 3,
        },
        /* 76 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 77 */ Instruction {
            execute: inst_ld_mem_ix_im8_a,
            clock_cycles: 19,
            size: 3,
        },
        /* 78 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 79 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7C */ Instruction {
            execute: inst_ld_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 7D */ Instruction {
            execute: inst_ld_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 7E */ Instruction {
            execute: inst_ld_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 7F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 80 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 81 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 82 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 83 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 84 */ Instruction {
            execute: inst_add_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 85 */ Instruction {
            execute: inst_add_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 86 */ Instruction {
            execute: inst_add_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 87 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 88 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 89 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8C */ Instruction {
            execute: inst_adc_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 8D */ Instruction {
            execute: inst_adc_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 8E */ Instruction {
            execute: inst_adc_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 8F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 90 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 91 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 92 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 93 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 94 */ Instruction {
            execute: inst_sub_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 95 */ Instruction {
            execute: inst_sub_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 96 */ Instruction {
            execute: inst_sub_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 97 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 98 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 99 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9C */ Instruction {
            execute: inst_sbc_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* 9D */ Instruction {
            execute: inst_sbc_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* 9E */ Instruction {
            execute: inst_sbc_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 9F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A4 */ Instruction {
            execute: inst_and_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* A5 */ Instruction {
            execute: inst_and_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* A6 */ Instruction {
            execute: inst_and_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* A7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AC */ Instruction {
            execute: inst_xor_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* AD */ Instruction {
            execute: inst_xor_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* AE */ Instruction {
            execute: inst_xor_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* AF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B4 */ Instruction {
            execute: inst_or_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* B5 */ Instruction {
            execute: inst_or_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* B6 */ Instruction {
            execute: inst_or_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* B7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BC */ Instruction {
            execute: inst_cp_a_ixh,
            clock_cycles: 8,
            size: 2,
        },
        /* BD */ Instruction {
            execute: inst_cp_a_ixl,
            clock_cycles: 8,
            size: 2,
        },
        /* BE */ Instruction {
            execute: inst_cp_a_mem_ix_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* BF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CB */ Instruction {  // This is the bit manip. instruction prefix.
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* CC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E1 */ Instruction {
            execute: inst_pop_ix,
            clock_cycles: 14,
            size: 2,
        },
        /* E2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E3 */ Instruction {
            execute: inst_ex_mem_sp_ix,
            clock_cycles: 23,
            size: 2,
        },
        /* E4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E5 */ Instruction {
            execute: inst_push_ix,
            clock_cycles: 15,
            size: 2,
        },
        /* E6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E9 */ Instruction {
            execute: inst_jp_ix,
            clock_cycles: 8,
            size: 2,
        },
        /* EA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* ED */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F9 */ Instruction {
            execute: inst_ld_sp_ix,
            clock_cycles: 10,
            size: 2,
        },
        /* FA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
    ],
    // DDCB-Prefixed (IX bit manipulation) instructions:
    ix_bit: [
        /* 00 */ Instruction {
            execute: inst_rlc_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 01 */ Instruction {
            execute: inst_rlc_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 02 */ Instruction {
            execute: inst_rlc_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 03 */ Instruction {
            execute: inst_rlc_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 04 */ Instruction {
            execute: inst_rlc_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 05 */ Instruction {
            execute: inst_rlc_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 06 */ Instruction {
            execute: inst_rlc_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 07 */ Instruction {
            execute: inst_rlc_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 08 */ Instruction {
            execute: inst_rrc_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 09 */ Instruction {
            execute: inst_rrc_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 0A */ Instruction {
            execute: inst_rrc_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 0B */ Instruction {
            execute: inst_rrc_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 0C */ Instruction {
            execute: inst_rrc_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 0D */ Instruction {
            execute: inst_rrc_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 0E */ Instruction {
            execute: inst_rrc_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 0F */ Instruction {
            execute: inst_rrc_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 10 */ Instruction {
            execute: inst_rl_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 11 */ Instruction {
            execute: inst_rl_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 12 */ Instruction {
            execute: inst_rl_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 13 */ Instruction {
            execute: inst_rl_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 14 */ Instruction {
            execute: inst_rl_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 15 */ Instruction {
            execute: inst_rl_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 16 */ Instruction {
            execute: inst_rl_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 17 */ Instruction {
            execute: inst_rl_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 18 */ Instruction {
            execute: inst_rr_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 19 */ Instruction {
            execute: inst_rr_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 1A */ Instruction {
            execute: inst_rr_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 1B */ Instruction {
            execute: inst_rr_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 1C */ Instruction {
            execute: inst_rr_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 1D */ Instruction {
            execute: inst_rr_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 1E */ Instruction {
            execute: inst_rr_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 1F */ Instruction {
            execute: inst_rr_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 20 */ Instruction {
            execute: inst_sla_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 21 */ Instruction {
            execute: inst_sla_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 22 */ Instruction {
            execute: inst_sla_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 23 */ Instruction {
            execute: inst_sla_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 24 */ Instruction {
            execute: inst_sla_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 25 */ Instruction {
            execute: inst_sla_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 26 */ Instruction {
            execute: inst_sla_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 27 */ Instruction {
            execute: inst_sla_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 28 */ Instruction {
            execute: inst_sra_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 29 */ Instruction {
            execute: inst_sra_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 2A */ Instruction {
            execute: inst_sra_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 2B */ Instruction {
            execute: inst_sra_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 2C */ Instruction {
            execute: inst_sra_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 2D */ Instruction {
            execute: inst_sra_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 2E */ Instruction {
            execute: inst_sra_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 2F */ Instruction {
            execute: inst_sra_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 30 */ Instruction {
            execute: inst_sll_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 31 */ Instruction {
            execute: inst_sll_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 32 */ Instruction {
            execute: inst_sll_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 33 */ Instruction {
            execute: inst_sll_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 34 */ Instruction {
            execute: inst_sll_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 35 */ Instruction {
            execute: inst_sll_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 36 */ Instruction {
            execute: inst_sll_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 37 */ Instruction {
            execute: inst_sll_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 38 */ Instruction {
            execute: inst_srl_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 39 */ Instruction {
            execute: inst_srl_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 3A */ Instruction {
            execute: inst_srl_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 3B */ Instruction {
            execute: inst_srl_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 3C */ Instruction {
            execute: inst_srl_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 3D */ Instruction {
            execute: inst_srl_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 3E */ Instruction {
            execute: inst_srl_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 3F */ Instruction {
            execute: inst_srl_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 40 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 41 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 42 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 43 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 44 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 45 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 46 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 47 */ Instruction {
            execute: inst_bit_0_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 48 */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 49 */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4A */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4B */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4C */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4D */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4E */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4F */ Instruction {
            execute: inst_bit_1_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 50 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 51 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 52 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 53 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 54 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 55 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 56 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 57 */ Instruction {
            execute: inst_bit_2_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 58 */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 59 */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5A */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5B */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5C */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5D */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5E */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5F */ Instruction {
            execute: inst_bit_3_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 60 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 61 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 62 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 63 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 64 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 65 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 66 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 67 */ Instruction {
            execute: inst_bit_4_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 68 */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 69 */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6A */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6B */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6C */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6D */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6E */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6F */ Instruction {
            execute: inst_bit_5_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 70 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 71 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 72 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 73 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 74 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 75 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 76 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 77 */ Instruction {
            execute: inst_bit_6_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 78 */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 79 */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7A */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7B */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7C */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7D */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7E */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7F */ Instruction {
            execute: inst_bit_7_mem_ix_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 80 */ Instruction {
            execute: inst_res_0_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 81 */ Instruction {
            execute: inst_res_0_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 82 */ Instruction {
            execute: inst_res_0_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 83 */ Instruction {
            execute: inst_res_0_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 84 */ Instruction {
            execute: inst_res_0_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 85 */ Instruction {
            execute: inst_res_0_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 86 */ Instruction {
            execute: inst_res_0_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 87 */ Instruction {
            execute: inst_res_0_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 88 */ Instruction {
            execute: inst_res_1_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 89 */ Instruction {
            execute: inst_res_1_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 8A */ Instruction {
            execute: inst_res_1_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 8B */ Instruction {
            execute: inst_res_1_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 8C */ Instruction {
            execute: inst_res_1_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 8D */ Instruction {
            execute: inst_res_1_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 8E */ Instruction {
            execute: inst_res_1_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 8F */ Instruction {
            execute: inst_res_1_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 90 */ Instruction {
            execute: inst_res_2_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 91 */ Instruction {
            execute: inst_res_2_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 92 */ Instruction {
            execute: inst_res_2_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 93 */ Instruction {
            execute: inst_res_2_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 94 */ Instruction {
            execute: inst_res_2_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 95 */ Instruction {
            execute: inst_res_2_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 96 */ Instruction {
            execute: inst_res_2_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 97 */ Instruction {
            execute: inst_res_2_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 98 */ Instruction {
            execute: inst_res_3_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 99 */ Instruction {
            execute: inst_res_3_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 9A */ Instruction {
            execute: inst_res_3_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 9B */ Instruction {
            execute: inst_res_3_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 9C */ Instruction {
            execute: inst_res_3_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 9D */ Instruction {
            execute: inst_res_3_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 9E */ Instruction {
            execute: inst_res_3_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 9F */ Instruction {
            execute: inst_res_3_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* A0 */ Instruction {
            execute: inst_res_4_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* A1 */ Instruction {
            execute: inst_res_4_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* A2 */ Instruction {
            execute: inst_res_4_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* A3 */ Instruction {
            execute: inst_res_4_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* A4 */ Instruction {
            execute: inst_res_4_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* A5 */ Instruction {
            execute: inst_res_4_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* A6 */ Instruction {
            execute: inst_res_4_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* A7 */ Instruction {
            execute: inst_res_4_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* A8 */ Instruction {
            execute: inst_res_5_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* A9 */ Instruction {
            execute: inst_res_5_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* AA */ Instruction {
            execute: inst_res_5_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* AB */ Instruction {
            execute: inst_res_5_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* AC */ Instruction {
            execute: inst_res_5_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* AD */ Instruction {
            execute: inst_res_5_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* AE */ Instruction {
            execute: inst_res_5_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* AF */ Instruction {
            execute: inst_res_5_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* B0 */ Instruction {
            execute: inst_res_6_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* B1 */ Instruction {
            execute: inst_res_6_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* B2 */ Instruction {
            execute: inst_res_6_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* B3 */ Instruction {
            execute: inst_res_6_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* B4 */ Instruction {
            execute: inst_res_6_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* B5 */ Instruction {
            execute: inst_res_6_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* B6 */ Instruction {
            execute: inst_res_6_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* B7 */ Instruction {
            execute: inst_res_6_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* B8 */ Instruction {
            execute: inst_res_7_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* B9 */ Instruction {
            execute: inst_res_7_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* BA */ Instruction {
            execute: inst_res_7_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* BB */ Instruction {
            execute: inst_res_7_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* BC */ Instruction {
            execute: inst_res_7_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* BD */ Instruction {
            execute: inst_res_7_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* BE */ Instruction {
            execute: inst_res_7_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* BF */ Instruction {
            execute: inst_res_7_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* C0 */ Instruction {
            execute: inst_set_0_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* C1 */ Instruction {
            execute: inst_set_0_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* C2 */ Instruction {
            execute: inst_set_0_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* C3 */ Instruction {
            execute: inst_set_0_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* C4 */ Instruction {
            execute: inst_set_0_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* C5 */ Instruction {
            execute: inst_set_0_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* C6 */ Instruction {
            execute: inst_set_0_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* C7 */ Instruction {
            execute: inst_set_0_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* C8 */ Instruction {
            execute: inst_set_1_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* C9 */ Instruction {
            execute: inst_set_1_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* CA */ Instruction {
            execute: inst_set_1_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* CB */ Instruction {
            execute: inst_set_1_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* CC */ Instruction {
            execute: inst_set_1_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* CD */ Instruction {
            execute: inst_set_1_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* CE */ Instruction {
            execute: inst_set_1_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* CF */ Instruction {
            execute: inst_set_1_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* D0 */ Instruction {
            execute: inst_set_2_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* D1 */ Instruction {
            execute: inst_set_2_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* D2 */ Instruction {
            execute: inst_set_2_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* D3 */ Instruction {
            execute: inst_set_2_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* D4 */ Instruction {
            execute: inst_set_2_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* D5 */ Instruction {
            execute: inst_set_2_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* D6 */ Instruction {
            execute: inst_set_2_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* D7 */ Instruction {
            execute: inst_set_2_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* D8 */ Instruction {
            execute: inst_set_3_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* D9 */ Instruction {
            execute: inst_set_3_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* DA */ Instruction {
            execute: inst_set_3_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* DB */ Instruction {
            execute: inst_set_3_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* DC */ Instruction {
            execute: inst_set_3_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* DD */ Instruction {
            execute: inst_set_3_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* DE */ Instruction {
            execute: inst_set_3_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* DF */ Instruction {
            execute: inst_set_3_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* E0 */ Instruction {
            execute: inst_set_4_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* E1 */ Instruction {
            execute: inst_set_4_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* E2 */ Instruction {
            execute: inst_set_4_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* E3 */ Instruction {
            execute: inst_set_4_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* E4 */ Instruction {
            execute: inst_set_4_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* E5 */ Instruction {
            execute: inst_set_4_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* E6 */ Instruction {
            execute: inst_set_4_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* E7 */ Instruction {
            execute: inst_set_4_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* E8 */ Instruction {
            execute: inst_set_5_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* E9 */ Instruction {
            execute: inst_set_5_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* EA */ Instruction {
            execute: inst_set_5_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* EB */ Instruction {
            execute: inst_set_5_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* EC */ Instruction {
            execute: inst_set_5_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* ED */ Instruction {
            execute: inst_set_5_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* EE */ Instruction {
            execute: inst_set_5_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* EF */ Instruction {
            execute: inst_set_5_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* F0 */ Instruction {
            execute: inst_set_6_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* F1 */ Instruction {
            execute: inst_set_6_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* F2 */ Instruction {
            execute: inst_set_6_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* F3 */ Instruction {
            execute: inst_set_6_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* F4 */ Instruction {
            execute: inst_set_6_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* F5 */ Instruction {
            execute: inst_set_6_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* F6 */ Instruction {
            execute: inst_set_6_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* F7 */ Instruction {
            execute: inst_set_6_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* F8 */ Instruction {
            execute: inst_set_7_mem_ix_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* F9 */ Instruction {
            execute: inst_set_7_mem_ix_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* FA */ Instruction {
            execute: inst_set_7_mem_ix_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* FB */ Instruction {
            execute: inst_set_7_mem_ix_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* FC */ Instruction {
            execute: inst_set_7_mem_ix_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* FD */ Instruction {
            execute: inst_set_7_mem_ix_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* FE */ Instruction {
            execute: inst_set_7_mem_ix_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* FF */ Instruction {
            execute: inst_set_7_mem_ix_im8_a,
            clock_cycles: 23,
            size: 4,
        },
    ],
    // FD-Prefixed (IY) instructions:

    // FD acts as a modifier prefix, therefore if it doesn't affect an
    // instruction's operation, it is interpreted as a NOP.
    iy: [
        /* 00 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 01 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 02 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 03 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 04 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 05 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 06 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 07 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 08 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 09 */ Instruction {
            execute: inst_add_iy_bc,
            clock_cycles: 15,
            size: 2,
        },
        /* 0A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 0F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 10 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 11 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 12 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 13 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 14 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 15 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 16 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 17 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 18 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 19 */ Instruction {
            execute: inst_add_iy_de,
            clock_cycles: 15,
            size: 2,
        },
        /* 1A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 1F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 20 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 21 */ Instruction {
            execute: inst_ld_iy_im16,
            clock_cycles: 14,
            size: 4,
        },
        /* 22 */ Instruction {
            execute: inst_ld_mem_im16_iy,
            clock_cycles: 20,
            size: 4,
        },
        /* 23 */ Instruction {
            execute: inst_inc_iy,
            clock_cycles: 10,
            size: 2,
        },
        /* 24 */ Instruction {
            execute: inst_inc_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 25 */ Instruction {
            execute: inst_dec_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 26 */ Instruction {
            execute: inst_ld_iyh_im8,
            clock_cycles: 11,
            size: 3,
        },
        /* 27 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 28 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 29 */ Instruction {
            execute: inst_add_iy_iy,
            clock_cycles: 15,
            size: 2,
        },
        /* 2A */ Instruction {
            execute: inst_ld_iy_mem_im16,
            clock_cycles: 20,
            size: 4,
        },
        /* 2B */ Instruction {
            execute: inst_dec_iy,
            clock_cycles: 10,
            size: 2,
        },
        /* 2C */ Instruction {
            execute: inst_inc_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 2D */ Instruction {
            execute: inst_dec_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 2E */ Instruction {
            execute: inst_ld_iyl_im8,
            clock_cycles: 11,
            size: 3,
        },
        /* 2F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 30 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 31 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 32 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 33 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 34 */ Instruction {
            execute: inst_inc_mem_iy_im8,
            clock_cycles: 23,
            size: 3,
        },
        /* 35 */ Instruction {
            execute: inst_dec_mem_iy_im8,
            clock_cycles: 23,
            size: 3,
        },
        /* 36 */ Instruction {
            execute: inst_ld_mem_iy_im8_im8,
            clock_cycles: 19,
            size: 4,
        },
        /* 37 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 38 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 39 */ Instruction {
            execute: inst_add_iy_sp,
            clock_cycles: 15,
            size: 2,
        },
        /* 3A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3C */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3D */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3E */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 3F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 40 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 41 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 42 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 43 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 44 */ Instruction {
            execute: inst_ld_b_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 45 */ Instruction {
            execute: inst_ld_b_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 46 */ Instruction {
            execute: inst_ld_b_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 47 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 48 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 49 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 4C */ Instruction {
            execute: inst_ld_c_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 4D */ Instruction {
            execute: inst_ld_c_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 4E */ Instruction {
            execute: inst_ld_c_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 4F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 50 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 51 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 52 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 53 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 54 */ Instruction {
            execute: inst_ld_d_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 55 */ Instruction {
            execute: inst_ld_d_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 56 */ Instruction {
            execute: inst_ld_d_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 57 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 58 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 59 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 5C */ Instruction {
            execute: inst_ld_e_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 5D */ Instruction {
            execute: inst_ld_e_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 5E */ Instruction {
            execute: inst_ld_e_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 5F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 60 */ Instruction {
            execute: inst_ld_iyh_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 61 */ Instruction {
            execute: inst_ld_iyh_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 62 */ Instruction {
            execute: inst_ld_iyh_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 63 */ Instruction {
            execute: inst_ld_iyh_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 64 */ Instruction {
            execute: inst_ld_iyh_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 65 */ Instruction {
            execute: inst_ld_iyh_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 66 */ Instruction {
            execute: inst_ld_h_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 67 */ Instruction {
            execute: inst_ld_iyh_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 68 */ Instruction {
            execute: inst_ld_iyl_b,
            clock_cycles: 8,
            size: 2,
        },
        /* 69 */ Instruction {
            execute: inst_ld_iyl_c,
            clock_cycles: 8,
            size: 2,
        },
        /* 6A */ Instruction {
            execute: inst_ld_iyl_d,
            clock_cycles: 8,
            size: 2,
        },
        /* 6B */ Instruction {
            execute: inst_ld_iyl_e,
            clock_cycles: 8,
            size: 2,
        },
        /* 6C */ Instruction {
            execute: inst_ld_iyl_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 6D */ Instruction {
            execute: inst_ld_iyl_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 6E */ Instruction {
            execute: inst_ld_l_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 6F */ Instruction {
            execute: inst_ld_iyl_a,
            clock_cycles: 8,
            size: 2,
        },
        /* 70 */ Instruction {
            execute: inst_ld_mem_iy_im8_b,
            clock_cycles: 19,
            size: 3,
        },
        /* 71 */ Instruction {
            execute: inst_ld_mem_iy_im8_c,
            clock_cycles: 19,
            size: 3,
        },
        /* 72 */ Instruction {
            execute: inst_ld_mem_iy_im8_d,
            clock_cycles: 19,
            size: 3,
        },
        /* 73 */ Instruction {
            execute: inst_ld_mem_iy_im8_e,
            clock_cycles: 19,
            size: 3,
        },
        /* 74 */ Instruction {
            execute: inst_ld_mem_iy_im8_h,
            clock_cycles: 19,
            size: 3,
        },
        /* 75 */ Instruction {
            execute: inst_ld_mem_iy_im8_l,
            clock_cycles: 19,
            size: 3,
        },
        /* 76 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 77 */ Instruction {
            execute: inst_ld_mem_iy_im8_a,
            clock_cycles: 19,
            size: 3,
        },
        /* 78 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 79 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 7C */ Instruction {
            execute: inst_ld_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 7D */ Instruction {
            execute: inst_ld_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 7E */ Instruction {
            execute: inst_ld_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 7F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 80 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 81 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 82 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 83 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 84 */ Instruction {
            execute: inst_add_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 85 */ Instruction {
            execute: inst_add_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 86 */ Instruction {
            execute: inst_add_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 87 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 88 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 89 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 8C */ Instruction {
            execute: inst_adc_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 8D */ Instruction {
            execute: inst_adc_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 8E */ Instruction {
            execute: inst_adc_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 8F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 90 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 91 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 92 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 93 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 94 */ Instruction {
            execute: inst_sub_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 95 */ Instruction {
            execute: inst_sub_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 96 */ Instruction {
            execute: inst_sub_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 97 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 98 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 99 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9A */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9B */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* 9C */ Instruction {
            execute: inst_sbc_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* 9D */ Instruction {
            execute: inst_sbc_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* 9E */ Instruction {
            execute: inst_sbc_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* 9F */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A4 */ Instruction {
            execute: inst_and_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* A5 */ Instruction {
            execute: inst_and_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* A6 */ Instruction {
            execute: inst_and_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* A7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* A9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* AC */ Instruction {
            execute: inst_xor_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* AD */ Instruction {
            execute: inst_xor_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* AE */ Instruction {
            execute: inst_xor_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* AF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B4 */ Instruction {
            execute: inst_or_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* B5 */ Instruction {
            execute: inst_or_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* B6 */ Instruction {
            execute: inst_or_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* B7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* B9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* BC */ Instruction {
            execute: inst_cp_a_iyh,
            clock_cycles: 8,
            size: 2,
        },
        /* BD */ Instruction {
            execute: inst_cp_a_iyl,
            clock_cycles: 8,
            size: 2,
        },
        /* BE */ Instruction {
            execute: inst_cp_a_mem_iy_im8,
            clock_cycles: 19,
            size: 3,
        },
        /* BF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* C9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CB */ Instruction {  // This is the bit manip. instruction prefix.
            execute: inst_nop2,
            clock_cycles: 8,
            size: 2,
        },
        /* CC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* CF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* D9 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* DF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E1 */ Instruction {
            execute: inst_pop_iy,
            clock_cycles: 14,
            size: 2,
        },
        /* E2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E3 */ Instruction {
            execute: inst_ex_mem_sp_iy,
            clock_cycles: 23,
            size: 2,
        },
        /* E4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E5 */ Instruction {
            execute: inst_push_iy,
            clock_cycles: 15,
            size: 2,
        },
        /* E6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* E9 */ Instruction {
            execute: inst_jp_iy,
            clock_cycles: 8,
            size: 2,
        },
        /* EA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* ED */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* EF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F0 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F1 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F2 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F3 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F4 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F5 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F6 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F7 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F8 */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* F9 */ Instruction {
            execute: inst_ld_sp_iy,
            clock_cycles: 10,
            size: 2,
        },
        /* FA */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FB */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FC */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FD */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FE */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
        /* FF */ Instruction {
            execute: inst_nop1,
            clock_cycles: 4,
            size: 1,
        },
    ],
    // FDCB-Prefixed (IY bit manipulation) instructions:
    iy_bit: [
        /* 00 */ Instruction {
            execute: inst_rlc_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 01 */ Instruction {
            execute: inst_rlc_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 02 */ Instruction {
            execute: inst_rlc_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 03 */ Instruction {
            execute: inst_rlc_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 04 */ Instruction {
            execute: inst_rlc_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 05 */ Instruction {
            execute: inst_rlc_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 06 */ Instruction {
            execute: inst_rlc_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 07 */ Instruction {
            execute: inst_rlc_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 08 */ Instruction {
            execute: inst_rrc_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 09 */ Instruction {
            execute: inst_rrc_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 0A */ Instruction {
            execute: inst_rrc_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 0B */ Instruction {
            execute: inst_rrc_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 0C */ Instruction {
            execute: inst_rrc_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 0D */ Instruction {
            execute: inst_rrc_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 0E */ Instruction {
            execute: inst_rrc_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 0F */ Instruction {
            execute: inst_rrc_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 10 */ Instruction {
            execute: inst_rl_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 11 */ Instruction {
            execute: inst_rl_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 12 */ Instruction {
            execute: inst_rl_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 13 */ Instruction {
            execute: inst_rl_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 14 */ Instruction {
            execute: inst_rl_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 15 */ Instruction {
            execute: inst_rl_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 16 */ Instruction {
            execute: inst_rl_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 17 */ Instruction {
            execute: inst_rl_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 18 */ Instruction {
            execute: inst_rr_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 19 */ Instruction {
            execute: inst_rr_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 1A */ Instruction {
            execute: inst_rr_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 1B */ Instruction {
            execute: inst_rr_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 1C */ Instruction {
            execute: inst_rr_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 1D */ Instruction {
            execute: inst_rr_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 1E */ Instruction {
            execute: inst_rr_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 1F */ Instruction {
            execute: inst_rr_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 20 */ Instruction {
            execute: inst_sla_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 21 */ Instruction {
            execute: inst_sla_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 22 */ Instruction {
            execute: inst_sla_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 23 */ Instruction {
            execute: inst_sla_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 24 */ Instruction {
            execute: inst_sla_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 25 */ Instruction {
            execute: inst_sla_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 26 */ Instruction {
            execute: inst_sla_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 27 */ Instruction {
            execute: inst_sla_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 28 */ Instruction {
            execute: inst_sra_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 29 */ Instruction {
            execute: inst_sra_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 2A */ Instruction {
            execute: inst_sra_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 2B */ Instruction {
            execute: inst_sra_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 2C */ Instruction {
            execute: inst_sra_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 2D */ Instruction {
            execute: inst_sra_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 2E */ Instruction {
            execute: inst_sra_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 2F */ Instruction {
            execute: inst_sra_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 30 */ Instruction {
            execute: inst_sll_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 31 */ Instruction {
            execute: inst_sll_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 32 */ Instruction {
            execute: inst_sll_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 33 */ Instruction {
            execute: inst_sll_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 34 */ Instruction {
            execute: inst_sll_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 35 */ Instruction {
            execute: inst_sll_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 36 */ Instruction {
            execute: inst_sll_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 37 */ Instruction {
            execute: inst_sll_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 38 */ Instruction {
            execute: inst_srl_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 39 */ Instruction {
            execute: inst_srl_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 3A */ Instruction {
            execute: inst_srl_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 3B */ Instruction {
            execute: inst_srl_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 3C */ Instruction {
            execute: inst_srl_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 3D */ Instruction {
            execute: inst_srl_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 3E */ Instruction {
            execute: inst_srl_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 3F */ Instruction {
            execute: inst_srl_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 40 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 41 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 42 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 43 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 44 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 45 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 46 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 47 */ Instruction {
            execute: inst_bit_0_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 48 */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 49 */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4A */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4B */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4C */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4D */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4E */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 4F */ Instruction {
            execute: inst_bit_1_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 50 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 51 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 52 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 53 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 54 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 55 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 56 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 57 */ Instruction {
            execute: inst_bit_2_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 58 */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 59 */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5A */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5B */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5C */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5D */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5E */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 5F */ Instruction {
            execute: inst_bit_3_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 60 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 61 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 62 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 63 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 64 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 65 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 66 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 67 */ Instruction {
            execute: inst_bit_4_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 68 */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 69 */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6A */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6B */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6C */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6D */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6E */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 6F */ Instruction {
            execute: inst_bit_5_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 70 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 71 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 72 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 73 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 74 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 75 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 76 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 77 */ Instruction {
            execute: inst_bit_6_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 78 */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 79 */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7A */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7B */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7C */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7D */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7E */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 7F */ Instruction {
            execute: inst_bit_7_mem_iy_im8,
            clock_cycles: 20,
            size: 4,
        },
        /* 80 */ Instruction {
            execute: inst_res_0_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 81 */ Instruction {
            execute: inst_res_0_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 82 */ Instruction {
            execute: inst_res_0_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 83 */ Instruction {
            execute: inst_res_0_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 84 */ Instruction {
            execute: inst_res_0_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 85 */ Instruction {
            execute: inst_res_0_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 86 */ Instruction {
            execute: inst_res_0_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 87 */ Instruction {
            execute: inst_res_0_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 88 */ Instruction {
            execute: inst_res_1_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 89 */ Instruction {
            execute: inst_res_1_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 8A */ Instruction {
            execute: inst_res_1_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 8B */ Instruction {
            execute: inst_res_1_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 8C */ Instruction {
            execute: inst_res_1_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 8D */ Instruction {
            execute: inst_res_1_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 8E */ Instruction {
            execute: inst_res_1_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 8F */ Instruction {
            execute: inst_res_1_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 90 */ Instruction {
            execute: inst_res_2_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 91 */ Instruction {
            execute: inst_res_2_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 92 */ Instruction {
            execute: inst_res_2_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 93 */ Instruction {
            execute: inst_res_2_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 94 */ Instruction {
            execute: inst_res_2_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 95 */ Instruction {
            execute: inst_res_2_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 96 */ Instruction {
            execute: inst_res_2_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 97 */ Instruction {
            execute: inst_res_2_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* 98 */ Instruction {
            execute: inst_res_3_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* 99 */ Instruction {
            execute: inst_res_3_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* 9A */ Instruction {
            execute: inst_res_3_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* 9B */ Instruction {
            execute: inst_res_3_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* 9C */ Instruction {
            execute: inst_res_3_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* 9D */ Instruction {
            execute: inst_res_3_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* 9E */ Instruction {
            execute: inst_res_3_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* 9F */ Instruction {
            execute: inst_res_3_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* A0 */ Instruction {
            execute: inst_res_4_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* A1 */ Instruction {
            execute: inst_res_4_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* A2 */ Instruction {
            execute: inst_res_4_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* A3 */ Instruction {
            execute: inst_res_4_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* A4 */ Instruction {
            execute: inst_res_4_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* A5 */ Instruction {
            execute: inst_res_4_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* A6 */ Instruction {
            execute: inst_res_4_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* A7 */ Instruction {
            execute: inst_res_4_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* A8 */ Instruction {
            execute: inst_res_5_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* A9 */ Instruction {
            execute: inst_res_5_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* AA */ Instruction {
            execute: inst_res_5_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* AB */ Instruction {
            execute: inst_res_5_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* AC */ Instruction {
            execute: inst_res_5_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* AD */ Instruction {
            execute: inst_res_5_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* AE */ Instruction {
            execute: inst_res_5_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* AF */ Instruction {
            execute: inst_res_5_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* B0 */ Instruction {
            execute: inst_res_6_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* B1 */ Instruction {
            execute: inst_res_6_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* B2 */ Instruction {
            execute: inst_res_6_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* B3 */ Instruction {
            execute: inst_res_6_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* B4 */ Instruction {
            execute: inst_res_6_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* B5 */ Instruction {
            execute: inst_res_6_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* B6 */ Instruction {
            execute: inst_res_6_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* B7 */ Instruction {
            execute: inst_res_6_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* B8 */ Instruction {
            execute: inst_res_7_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* B9 */ Instruction {
            execute: inst_res_7_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* BA */ Instruction {
            execute: inst_res_7_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* BB */ Instruction {
            execute: inst_res_7_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* BC */ Instruction {
            execute: inst_res_7_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* BD */ Instruction {
            execute: inst_res_7_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* BE */ Instruction {
            execute: inst_res_7_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* BF */ Instruction {
            execute: inst_res_7_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* C0 */ Instruction {
            execute: inst_set_0_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* C1 */ Instruction {
            execute: inst_set_0_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* C2 */ Instruction {
            execute: inst_set_0_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* C3 */ Instruction {
            execute: inst_set_0_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* C4 */ Instruction {
            execute: inst_set_0_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* C5 */ Instruction {
            execute: inst_set_0_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* C6 */ Instruction {
            execute: inst_set_0_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* C7 */ Instruction {
            execute: inst_set_0_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* C8 */ Instruction {
            execute: inst_set_1_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* C9 */ Instruction {
            execute: inst_set_1_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* CA */ Instruction {
            execute: inst_set_1_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* CB */ Instruction {
            execute: inst_set_1_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* CC */ Instruction {
            execute: inst_set_1_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* CD */ Instruction {
            execute: inst_set_1_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* CE */ Instruction {
            execute: inst_set_1_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* CF */ Instruction {
            execute: inst_set_1_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* D0 */ Instruction {
            execute: inst_set_2_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* D1 */ Instruction {
            execute: inst_set_2_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* D2 */ Instruction {
            execute: inst_set_2_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* D3 */ Instruction {
            execute: inst_set_2_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* D4 */ Instruction {
            execute: inst_set_2_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* D5 */ Instruction {
            execute: inst_set_2_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* D6 */ Instruction {
            execute: inst_set_2_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* D7 */ Instruction {
            execute: inst_set_2_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* D8 */ Instruction {
            execute: inst_set_3_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* D9 */ Instruction {
            execute: inst_set_3_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* DA */ Instruction {
            execute: inst_set_3_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* DB */ Instruction {
            execute: inst_set_3_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* DC */ Instruction {
            execute: inst_set_3_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* DD */ Instruction {
            execute: inst_set_3_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* DE */ Instruction {
            execute: inst_set_3_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* DF */ Instruction {
            execute: inst_set_3_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* E0 */ Instruction {
            execute: inst_set_4_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* E1 */ Instruction {
            execute: inst_set_4_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* E2 */ Instruction {
            execute: inst_set_4_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* E3 */ Instruction {
            execute: inst_set_4_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* E4 */ Instruction {
            execute: inst_set_4_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* E5 */ Instruction {
            execute: inst_set_4_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* E6 */ Instruction {
            execute: inst_set_4_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* E7 */ Instruction {
            execute: inst_set_4_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* E8 */ Instruction {
            execute: inst_set_5_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* E9 */ Instruction {
            execute: inst_set_5_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* EA */ Instruction {
            execute: inst_set_5_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* EB */ Instruction {
            execute: inst_set_5_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* EC */ Instruction {
            execute: inst_set_5_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* ED */ Instruction {
            execute: inst_set_5_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* EE */ Instruction {
            execute: inst_set_5_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* EF */ Instruction {
            execute: inst_set_5_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* F0 */ Instruction {
            execute: inst_set_6_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* F1 */ Instruction {
            execute: inst_set_6_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* F2 */ Instruction {
            execute: inst_set_6_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* F3 */ Instruction {
            execute: inst_set_6_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* F4 */ Instruction {
            execute: inst_set_6_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* F5 */ Instruction {
            execute: inst_set_6_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* F6 */ Instruction {
            execute: inst_set_6_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* F7 */ Instruction {
            execute: inst_set_6_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
        /* F8 */ Instruction {
            execute: inst_set_7_mem_iy_im8_b,
            clock_cycles: 23,
            size: 4,
        },
        /* F9 */ Instruction {
            execute: inst_set_7_mem_iy_im8_c,
            clock_cycles: 23,
            size: 4,
        },
        /* FA */ Instruction {
            execute: inst_set_7_mem_iy_im8_d,
            clock_cycles: 23,
            size: 4,
        },
        /* FB */ Instruction {
            execute: inst_set_7_mem_iy_im8_e,
            clock_cycles: 23,
            size: 4,
        },
        /* FC */ Instruction {
            execute: inst_set_7_mem_iy_im8_h,
            clock_cycles: 23,
            size: 4,
        },
        /* FD */ Instruction {
            execute: inst_set_7_mem_iy_im8_l,
            clock_cycles: 23,
            size: 4,
        },
        /* FE */ Instruction {
            execute: inst_set_7_mem_iy_im8,
            clock_cycles: 23,
            size: 4,
        },
        /* FF */ Instruction {
            execute: inst_set_7_mem_iy_im8_a,
            clock_cycles: 23,
            size: 4,
        },
    ],
};

// A function to load the instruction at the given address:
pub fn load_instruction(base: u16, memory: &mut memory::MemorySystem, cycle_timestamp: u32)
                       -> &'static Instruction {
    let first_byte = memory.read_byte(base, cycle_timestamp);
    match first_byte {
        0xCB => {
            let opcode = memory.read_byte(base + 1, cycle_timestamp);

            &INSTRUCTION_SET.bit[opcode as usize]
        },
        0xED => {
            let opcode = memory.read_byte(base + 1, cycle_timestamp);

            if (opcode >= 0x40) && (opcode <= 0x7F) {
                &INSTRUCTION_SET.extended[(opcode - 0x40) as usize]
            } else if (opcode >= 0xA0) && (opcode <= 0xBF) {
                &INSTRUCTION_SET.extended[(opcode - 0xA0 + 0x40) as usize]
            } else {
                &INSTRUCTION_SET.nop_2
            }
        },
        0xDD => {
            let second_byte = memory.read_byte(base + 1, cycle_timestamp);
            if second_byte == 0xCB {
                let opcode = memory.read_byte(base + 3, cycle_timestamp);
                &INSTRUCTION_SET.ix_bit[opcode as usize]
            } else {
                &INSTRUCTION_SET.ix[second_byte as usize]
            }
        },
        0xFD => {
            let second_byte = memory.read_byte(base + 1, cycle_timestamp);
            if second_byte == 0xCB {
                let opcode = memory.read_byte(base + 3, cycle_timestamp);
                &INSTRUCTION_SET.iy_bit[opcode as usize]
            } else {
                &INSTRUCTION_SET.iy[second_byte as usize]
            }
        },
        _ => {
            &INSTRUCTION_SET.main[first_byte as usize]
        },
    }
}

// Macros to make my life implementing the instructions easier:

// Pack the information from a flags structure into a flags byte.
macro_rules! pack_flags {
    ($flags_s:expr) => {
        (0 | if $flags_s.sign { cpu::FLAG_SIGN } else { 0 }
           | if $flags_s.zero { cpu::FLAG_ZERO } else { 0 }
           | if $flags_s.undoc_y { cpu::FLAG_UNDOC_Y } else { 0 }
           | if $flags_s.half_carry { cpu::FLAG_HALF_CARRY } else { 0 }
           | if $flags_s.undoc_x { cpu::FLAG_UNDOC_X } else { 0 }
           | if $flags_s.parity_overflow { cpu::FLAG_PARITY_OVERFLOW } else { 0 }
           | if $flags_s.add_sub { cpu::FLAG_ADD_SUB } else { 0 }
           | if $flags_s.carry { cpu::FLAG_CARRY } else { 0 }
        ) as u8
    };
}

// Unpack the information from a flags byte into a flags structure.
macro_rules! unpack_flags {
    ($flags_b_in:expr) => {
        {
            let flags_b: u8 = $flags_b_in;

            cpu::Z80Flags {
                sign:             (flags_b & cpu::FLAG_SIGN)            != 0,
                zero:             (flags_b & cpu::FLAG_ZERO)            != 0,
                undoc_y:          (flags_b & cpu::FLAG_UNDOC_Y)         != 0,
                half_carry:       (flags_b & cpu::FLAG_HALF_CARRY)      != 0,
                undoc_x:          (flags_b & cpu::FLAG_UNDOC_X)         != 0,
                parity_overflow:  (flags_b & cpu::FLAG_PARITY_OVERFLOW) != 0,
                add_sub:          (flags_b & cpu::FLAG_ADD_SUB)         != 0,
                carry:            (flags_b & cpu::FLAG_CARRY)           != 0,
            }
        }
    };
}

// Get the high byte of a 16-bit value:
macro_rules! get_high_of_16bit {
    ($value_in:expr) => {
        {
            let value: u16 = $value_in;

            (value >> 8) as u8
        }
    };
}

// Get the low byte of a 16-bit value:
macro_rules! get_low_of_16bit {
    ($value_in:expr) => {
        {
            let value: u16 = $value_in;

            (value & 0x00FF) as u8
        }
    };
}

// Set the high byte of a 16-bit value:
macro_rules! set_high_of_16bit {
    ($value:expr, $to_set_in:expr) => {
        {
            let _type_check: u16 = $value;
            let to_set: u8 = $to_set_in;

            $value = ($value & 0x00FF) | ((to_set as u16) << 8);
        }
    };
}

// Set the low byte of a 16-bit value:
macro_rules! set_low_of_16bit {
    ($value:expr, $to_set_in:expr) => {
        {
            let _type_check: u16 = $value;
            let to_set: u8 = $to_set_in;

            $value = ($value & 0xFF00) | (to_set as u16);
        }
    };
}

// Simple sign checks:
macro_rules! is_8bit_negative {
    ($value:expr) => {
        ($value & 0b1000_0000) != 0
    };
}
macro_rules! is_16bit_negative {
    ($value:expr) => {
        ($value & 0b1000_0000_0000_0000) != 0
    };
}

// Add to a 16-bit value a sign-extended 8-bit value.
macro_rules! add_16bit_signed_8bit {
    ($dest_in:expr, $to_add_in:expr) => {
        {
            let dest:   u16 = $dest_in;
            let to_add: u8  = $to_add_in;

            let to_add_ext: u16 = if (to_add & 0b1000_0000) != 0 {
                (to_add as u16) | 0xFF00
            } else {
                to_add as u16
            };

            (((dest as u32) + (to_add_ext as u32)) & 0xFFFF) as u16
        }
    };
}

// Check the parity of an 8-bit value.
macro_rules! even_parity_8bit {
    ($value_in:expr) =>  {
        {
            let value: u8 = $value_in;
            let mut count: u32 = 0;

            for iter in 0..8 {
                if (value & (1 << iter)) != 0 {
                    count += 1
                }
            }

            (count % 2) == 0
        }
    };
}

// Push a byte onto the stack:
macro_rules! stack_push_8bit {
    ($regs:expr, $memory:expr, $val:expr, $cycle_timestamp:expr) => {
        {
            $regs.sp -= 1;
            $memory.write_byte($regs.sp, $val, $cycle_timestamp);
        }
    };
}
// Pop a byte from the stack:
macro_rules! stack_pop_8bit {
    ($regs:expr, $memory:expr, $cycle_timestamp:expr) => {
        {
            let value = $memory.read_byte($regs.sp, $cycle_timestamp);
            $regs.sp += 1;

            value
        }
    };
}

// Push a 16-bit word onto the stack:
#[macro_export]
macro_rules! stack_push_16bit {
    ($regs:expr, $memory:expr, $val:expr, $cycle_timestamp:expr) => {
        {
            $regs.sp -= 2;
            $memory.write_word($regs.sp, $val, $cycle_timestamp);
        }
    };
}
// Pop a 16-bit word from the stack:
macro_rules! stack_pop_16bit {
    ($regs:expr, $memory:expr, $cycle_timestamp:expr) => {
        {
            let value = $memory.read_word($regs.sp, $cycle_timestamp);
            $regs.sp += 2;

            value
        }
    };
}

// Compose a 16-bit word out of two bytes:
macro_rules! compose_16bit_from_8bit {
    ($high_in:expr, $low_in:expr) => {
        {
            let high: u8 = $high_in;
            let low:  u8 = $low_in;

            (((high as u16) << 8) | (low as u16))
        }
    };
}

// Set the flags after an extended left-shift instruction:
macro_rules! apply_shift_left_flags {
    ($flags:expr, $old_val_in:expr, $new_val_in:expr) => {
        {
            let old_val: u8 = $old_val_in;
            let new_val: u8 = $new_val_in;

            $flags.half_carry = false;
            $flags.add_sub    = false;

            $flags.carry = (old_val & 0b1000_0000) != 0;
            $flags.zero = (new_val == 0);
            $flags.sign = is_8bit_negative!(new_val);
            $flags.parity_overflow = even_parity_8bit!(new_val);
        }
    };
}
// Set the flags after an extended right-shift instruction:
macro_rules! apply_shift_right_flags {
    ($flags:expr, $old_val_in:expr, $new_val_in:expr) => {
        {
            let old_val: u8 = $old_val_in;
            let new_val: u8 = $new_val_in;

            $flags.half_carry = false;
            $flags.add_sub    = false;

            $flags.carry = (old_val & 0b0000_0001) != 0;
            $flags.zero = (new_val == 0);
            $flags.sign = is_8bit_negative!(new_val);
            $flags.parity_overflow = even_parity_8bit!(new_val);
        }
    };
}
// Set the flags after a bit-checking instruction:
macro_rules! apply_inst_bit_flags {
    ($flags:expr, $order:expr, $value_in:expr) => {
        {
            let value: u8 = $value_in;

            $flags.half_carry = true;
            $flags.add_sub = false;
            $flags.zero = (value & (1 << $order)) == 0;
        }
    };
}

// Arithmetical operations:

// The following macros set the appropriate flags, and return the result:
macro_rules! add_8bit {
    ($flags:expr, $dest_in:expr, $to_add_in:expr, $plus_one_in:expr) => {
        {
            let dest:     u8   = $dest_in;
            let to_add:   u8   = $to_add_in;
            let plus_one: bool = $plus_one_in;

            let dest_int:   i32 =  dest   as i32;
            let to_add_int: i32 = (to_add as i32) + (if plus_one { 1 } else { 0 });
            let mut result_int = dest_int + to_add_int;
            if result_int > 0xFF {
                $flags.carry = true;
                result_int &= 0xFF;
            } else {
                $flags.carry = false;
            }
            $flags.zero = (result_int == 0);
            $flags.sign = is_8bit_negative!(result_int);
            $flags.parity_overflow = (!(is_8bit_negative!(to_add_int)) &&
                (!(is_8bit_negative!(dest_int)) && is_8bit_negative!(result_int))) ||

                (is_8bit_negative!(to_add_int) &&
                is_8bit_negative!(dest_int) && !(is_8bit_negative!(result_int)));
            $flags.half_carry = ((dest_int & 0x0F) + (to_add_int & 0x0F)) > 0x0F;
            $flags.add_sub = false;

            result_int as u8
        }
    };
}

macro_rules! inc_8bit {
    ($flags:expr, $dest_in:expr) => {
        {
            let dest:   u8 = $dest_in;
            let result: u8 = dest.wrapping_add(1);

            $flags.zero = (result == 0);
            $flags.sign = is_8bit_negative!(result);
            $flags.parity_overflow = (dest == 0x7F);
            $flags.half_carry = (dest & 0x0F) == 0x0F;
            $flags.add_sub = false;

            result
        }
    };
}

macro_rules! add_16bit {
    ($flags:expr, $dest_in:expr, $to_add_in:expr) => {
        {
            let dest:   u16 = $dest_in;
            let to_add: u16 = $to_add_in;

            let dest_int:   i32 = dest   as i32;
            let to_add_int: i32 = to_add as i32;

            let mut result_int = dest_int + to_add_int;

            if (result_int > 0xFFFF) {
                $flags.carry = true;
                result_int &= 0xFFFF;
            } else {
                $flags.carry = false;
            }
            $flags.half_carry = ((dest_int & 0x0FFF) + (to_add_int & 0x0FFF)) > 0x0FFF;
            $flags.add_sub = false;

            result_int as u16
        }
    };
}

// The 16-bit ADC instructions are different from the ADD instructions in that
// they are extended instructions added by ZiLOG, and unlike the 8080 16-bit
// ADD instructions, they set more flags.

macro_rules! add_16bit_carry {
    ($flags:expr, $dest_in:expr, $to_add_in:expr, $plus_one_in:expr) => {
        {
            let dest:     u16  = $dest_in;
            let to_add:   u16  = $to_add_in;
            let plus_one: bool = $plus_one_in;

            let dest_int:   i32 =  dest   as i32;
            let to_add_int: i32 = (to_add as i32) + (if plus_one { 1 } else { 0 });

            let mut result_int = dest_int + to_add_int;

            if (result_int > 0xFFFF) {
                $flags.carry = true;
                result_int &= 0xFFFF;
            } else {
                $flags.carry = false;
            }
            $flags.zero = (result_int == 0);
            $flags.sign = is_16bit_negative!(result_int);
            $flags.parity_overflow = (!(is_16bit_negative!(to_add_int)) &&
                (!(is_16bit_negative!(dest_int)) && is_16bit_negative!(result_int))) ||

                (is_16bit_negative!(to_add_int) &&
                is_16bit_negative!(dest_int) && !(is_16bit_negative!(result_int)));
            $flags.half_carry = ((dest_int & 0x0FFF) + (to_add_int & 0x0FFF)) > 0x0FFF;
            $flags.add_sub = false;

            result_int as u16
        }
    };
}

macro_rules! sub_8bit {
    ($flags:expr, $dest_in:expr, $to_sub_in:expr, $minus_one_in:expr) => {
        {
            let dest:      u8   = $dest_in;
            let to_sub:    u8   = $to_sub_in;
            let minus_one: bool = $minus_one_in;

            let dest_int:   i32 =  dest   as i32;
            let to_sub_int: i32 = (to_sub as i32) + (if minus_one { 1 } else { 0 });
            let mut result_int = dest_int - to_sub_int;
            if result_int < 0 {
                $flags.carry = true;
                result_int = ((result_int as u32) & 0xFF) as i32;
            } else {
                $flags.carry = false;
            }
            $flags.zero = (result_int == 0);
            $flags.sign = is_8bit_negative!(result_int);
            $flags.parity_overflow = (is_8bit_negative!(to_sub_int) &&
                (!(is_8bit_negative!(dest_int)) && is_8bit_negative!(result_int))) ||

                (!(is_8bit_negative!(to_sub_int)) &&
                is_8bit_negative!(dest_int) && !(is_8bit_negative!(result_int)));

            $flags.half_carry = ((dest_int & 0x0F) - (to_sub_int & 0x0F)) < 0;
            $flags.add_sub = true;

            result_int as u8
        }
    };
}

macro_rules! dec_8bit {
    ($flags:expr, $dest_in:expr) => {
        {
            let dest:   u8 = $dest_in;
            let result: u8 = dest.wrapping_sub(1);

            $flags.zero = (result == 0);
            $flags.sign = is_8bit_negative!(result);
            $flags.parity_overflow = (dest == 0x80);
            $flags.half_carry = (dest & 0x0F) == 0;
            $flags.add_sub = true;

            result
        }
    };
}

macro_rules! sub_16bit {
    ($flags:expr, $dest_in:expr, $to_sub_in:expr, $minus_one_in:expr) => {
        {
            let dest:      u16  = $dest_in;
            let to_sub:    u16  = $to_sub_in;
            let minus_one: bool = $minus_one_in;

            let dest_int:   i32 =  dest   as i32;
            let to_sub_int: i32 = (to_sub as i32) + (if minus_one { 1 } else { 0 });

            let mut result_int = dest_int - to_sub_int;

            if result_int < 0 {
                $flags.carry = true;
                result_int = ((result_int as u32) & 0xFFFF) as i32;
            } else {
                $flags.carry = false;
            }
            $flags.zero = (result_int == 0);
            $flags.sign = is_16bit_negative!(result_int);
            $flags.parity_overflow = (is_16bit_negative!(to_sub_int) &&
                (!(is_16bit_negative!(dest_int)) && is_16bit_negative!(result_int))) ||

                (!(is_16bit_negative!(to_sub_int)) &&
                is_16bit_negative!(dest_int) && !(is_16bit_negative!(result_int)));
            $flags.half_carry = ((dest_int & 0x0FFF) - (to_sub_int & 0x0FFF)) < 0;
            $flags.add_sub = true;

            result_int as u16
        }
    };
}
macro_rules! and_8bit {
    ($flags:expr, $dest_in:expr, $to_and_in:expr) => {
        {
            let dest:   u8  = $dest_in;
            let to_and: u8  = $to_and_in;

            let result = dest & to_and;

            $flags.half_carry = true;
            $flags.add_sub = false;
            $flags.carry = false;
            $flags.parity_overflow = even_parity_8bit!(result);
            $flags.zero = (result == 0);
            $flags.sign = is_8bit_negative!(result);

            result
        }
    };
}

macro_rules! or_8bit {
    ($flags:expr, $dest_in:expr, $to_or_in:expr) => {
        {
            let dest:  u8  = $dest_in;
            let to_or: u8  = $to_or_in;

            let result = dest | to_or;

            $flags.half_carry = false;
            $flags.add_sub = false;
            $flags.carry = false;
            $flags.parity_overflow = even_parity_8bit!(result);
            $flags.zero = (result == 0);
            $flags.sign = is_8bit_negative!(result);

            result
        }
    };
}

macro_rules! xor_8bit {
    ($flags:expr, $dest_in:expr, $to_xor_in:expr) => {
        {
            let dest:   u8  = $dest_in;
            let to_xor: u8  = $to_xor_in;

            let result = dest ^ to_xor;

            $flags.half_carry = false;
            $flags.add_sub = false;
            $flags.carry = false;
            $flags.parity_overflow = even_parity_8bit!(result);
            $flags.zero = (result == 0);
            $flags.sign = is_8bit_negative!(result);

            result
        }
    };
}


// No-ops:
fn inst_nop1(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.pc += 1;
}

fn inst_nop2(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.pc += 2;
}

// Main instructions:

// Jumps & INCs (0x00 to 0x3F):
fn inst_djnz_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val.wrapping_sub(1);

    set_high_of_16bit!(cpu.regs.bc, new_val);

    if new_val != 0 {
        let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
        cpu.added_delay = 5;
    } else {
        cpu.regs.pc += 2;
    }
}

fn inst_jr_nz_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.zero {
        let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
        cpu.added_delay = 5;
    } else {
        cpu.regs.pc += 2;
    }
}

fn inst_jr_nc_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.carry {
        let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
        cpu.added_delay = 5;
    } else {
        cpu.regs.pc += 2;
    }
}
fn inst_ld_bc_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.bc = imm;

    cpu.regs.pc += 3;
}
fn inst_ld_de_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.de = imm;

    cpu.regs.pc += 3;
}
fn inst_ld_hl_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.hl = imm;

    cpu.regs.pc += 3;
}
fn inst_ld_sp_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.sp = imm;

    cpu.regs.pc += 3;
}
fn inst_ld_mem_bc_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let to_write = cpu.regs.a;
    let address  = cpu.regs.bc;
    memory.write_byte(address, to_write, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_de_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let to_write = cpu.regs.a;
    let address  = cpu.regs.de;
    memory.write_byte(address, to_write, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_im16_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let to_write = cpu.regs.hl;
    let address  = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    memory.write_word(address, to_write, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_im16_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let to_write = cpu.regs.a;
    let address  = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    memory.write_byte(address, to_write, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_inc_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using an `add_?bit` function here, since these instructions
    // (8080 16-bit increment) don't affect any flags.
    let old_val = cpu.regs.bc;
    let new_val = old_val.wrapping_add(1);
    cpu.regs.bc = new_val;

    cpu.regs.pc += 1;
}
fn inst_inc_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using an `add_?bit` function here, since these instructions
    // (8080 16-bit increment) don't affect any flags.
    let old_val = cpu.regs.de;
    let new_val = old_val.wrapping_add(1);
    cpu.regs.de = new_val;

    cpu.regs.pc += 1;
}
fn inst_inc_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using an `add_?bit` function here, since these instructions
    // (8080 16-bit increment) don't affect any flags.
    let old_val = cpu.regs.hl;
    let new_val = old_val.wrapping_add(1);
    cpu.regs.hl = new_val;

    cpu.regs.pc += 1;
}
fn inst_inc_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using an `add_?bit` function here, since these instructions
    // (8080 16-bit increment) don't affect any flags.
    let old_val = cpu.regs.sp;
    let new_val = old_val.wrapping_add(1);
    cpu.regs.sp = new_val;

    cpu.regs.pc += 1;
}
fn inst_inc_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.bc, result);
    cpu.regs.pc += 1;
}
fn inst_inc_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.de, result);
    cpu.regs.pc += 1;
}
fn inst_inc_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.hl, result);
    cpu.regs.pc += 1;
}
fn inst_inc_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let address = cpu.regs.hl;
    let old_val = memory.read_byte(address, cpu.cycle_timestamp);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(address, result, cpu.cycle_timestamp);
    cpu.regs.pc += 1;
}
fn inst_dec_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.bc, result);
    cpu.regs.pc += 1;
}
fn inst_dec_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.de, result);
    cpu.regs.pc += 1;
}
fn inst_dec_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.hl, result);
    cpu.regs.pc += 1;
}
fn inst_dec_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let address = cpu.regs.hl;
    let old_val = memory.read_byte(address, cpu.cycle_timestamp);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(address, result, cpu.cycle_timestamp);
    cpu.regs.pc += 1;
}
fn inst_ld_b_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, imm);

    cpu.regs.pc += 2;
}
fn inst_ld_d_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, imm);

    cpu.regs.pc += 2;
}
fn inst_ld_h_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, imm);

    cpu.regs.pc += 2;
}
fn inst_ld_mem_hl_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let address = cpu.regs.hl;
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    memory.write_byte(address, imm, cpu.cycle_timestamp);

    cpu.regs.pc += 2;
}

fn inst_rlca(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };

    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.carry = (old_val & 0b1000_0000) != 0;

    cpu.regs.a = new_val;
    cpu.regs.pc += 1;
}

fn inst_rla(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };

    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.carry = (old_val & 0b1000_0000) != 0;

    cpu.regs.a = new_val;
    cpu.regs.pc += 1;
}

fn inst_daa(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;

    let mut to_add: u8 = 0;

    let high_digit = (old_val & 0xF0) >> 4;
    let low_digit  =  old_val & 0x0F;
   
    match cpu.regs.flags.add_sub {
        // Addition:
        false => {
            match !cpu.regs.flags.carry {
                // Carry not set:
                true =>  {
                    match !cpu.regs.flags.half_carry {
                        // Half-Carry not set:
                        true =>  {
                            if high_digit <= 0x9 && low_digit <= 0x9 {
                                to_add = 0;
                                cpu.regs.flags.carry = false;
                            } else if high_digit <= 0x8 &&
                                (low_digit >= 0xA && low_digit <= 0xF) {
                                to_add = 0x06;
                                cpu.regs.flags.carry = false;
                            } else if (high_digit >= 0xA && high_digit <= 0xF) &&
                                low_digit <= 0x9 {
                                to_add = 0x60;
                                cpu.regs.flags.carry = true;
                            } else if (high_digit >= 0x9 && high_digit <= 0xF) &&
                                (low_digit >= 0xA && low_digit <= 0xF) {
                                to_add = 0x66;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                        // Half-Carry set:
                        false => {
                            if high_digit <= 0x9 && low_digit <= 0x3 {
                                to_add = 0x06;
                                cpu.regs.flags.carry = false;
                            } else if (high_digit >= 0xA && high_digit <= 0xF) &&
                                low_digit <= 0x3 {
                                to_add = 0x66;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                    };
                },
                // Carry set:
                false => {
                    match !cpu.regs.flags.half_carry {
                        // Half-Carry not set:
                        true =>  {
                            if high_digit <= 0x2 && low_digit <= 0x9 {
                                to_add = 0x60;
                                cpu.regs.flags.carry = true;
                            } else if high_digit <= 0x2 &&
                                (low_digit >= 0xA && low_digit <= 0xF) {
                                to_add = 0x66;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                        // Half-Carry set:
                        false => {
                            if high_digit <= 0x3 && low_digit <= 0x3 {
                                to_add = 0x66;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                    };
                },
            };
        },
        // Subtraction:
        true =>  {
            match !cpu.regs.flags.carry {
                // Carry not set:
                true =>  {
                    match !cpu.regs.flags.half_carry {
                        // Half-Carry not set:
                        true =>  {
                            if high_digit <= 0x9 && low_digit <= 0x9 {
                                to_add = 0;
                                cpu.regs.flags.carry = false;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                        // Half-Carry set:
                        false => {
                            if high_digit <= 0x8 &&
                                (low_digit >= 0x6 && low_digit <= 0xF) {
                                to_add = 0xFA;
                                cpu.regs.flags.carry = false;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                    };
                },
                // Carry set:
                false => {
                    match !cpu.regs.flags.half_carry {
                        // Half-Carry not set:
                        true =>  {
                            if (high_digit >= 0x7 && high_digit <= 0xF) &&
                                low_digit <= 0x9 {
                                to_add = 0xA0;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                        // Half-Carry set:
                        false => {
                            if (high_digit >= 0x6 && high_digit <= 0x7) &&
                                (low_digit >= 0x6 && low_digit <= 0xF) {
                                to_add = 0x9A;
                                cpu.regs.flags.carry = true;
                            } else {
                                cpu.log_message("Warning: daa instruction failed.".to_owned());
                            }
                        },
                    };
                },
            };
        },
    };
    let new_val: u8 = old_val.wrapping_add(to_add);

    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    cpu.regs.a = new_val;
    cpu.regs.pc += 1;
}

fn inst_scf(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.flags.carry = true;
    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.pc += 1;
}

fn inst_ex_af_af_prime(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_a_new_a_prime = cpu.regs.a;
    let old_f_new_f_prime = cpu.regs.flags.clone();

    let old_a_prime_new_a = cpu.regs.a_prime;
    let old_f_prime_new_f = cpu.regs.flags_prime.clone();

    cpu.regs.a     = old_a_prime_new_a;
    cpu.regs.flags = old_f_prime_new_f;

    cpu.regs.a_prime     = old_a_new_a_prime;
    cpu.regs.flags_prime = old_f_new_f_prime;

    cpu.regs.pc += 1;
}

fn inst_jr_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
}

fn inst_jr_z_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.zero {
        let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
        cpu.added_delay = 5;
    } else {
        cpu.regs.pc += 2;
    }
}

fn inst_jr_c_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.carry {
        let offset = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.regs.pc = add_16bit_signed_8bit!(cpu.regs.pc + 2, offset);
        cpu.added_delay = 5;
    } else {
        cpu.regs.pc += 2;
    }
}

fn inst_add_hl_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_hl = cpu.regs.hl;
    let to_add = cpu.regs.bc;
    let result = add_16bit!(cpu.regs.flags, old_hl, to_add);

    cpu.regs.hl = result;
    cpu.regs.pc += 1;
}
fn inst_add_hl_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_hl = cpu.regs.hl;
    let to_add = cpu.regs.de;
    let result = add_16bit!(cpu.regs.flags, old_hl, to_add);

    cpu.regs.hl = result;
    cpu.regs.pc += 1;
}
fn inst_add_hl_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_hl = cpu.regs.hl;
    let to_add = cpu.regs.hl;
    let result = add_16bit!(cpu.regs.flags, old_hl, to_add);

    cpu.regs.hl = result;
    cpu.regs.pc += 1;
}
fn inst_add_hl_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_hl = cpu.regs.hl;
    let to_add = cpu.regs.sp;
    let result = add_16bit!(cpu.regs.flags, old_hl, to_add);

    cpu.regs.hl = result;
    cpu.regs.pc += 1;
}

fn inst_ld_a_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.a = val;
    cpu.regs.pc += 1;
}
fn inst_ld_a_mem_de(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.de;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.a = val;
    cpu.regs.pc += 1;
}
fn inst_ld_hl_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    let val = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.hl = val;
    cpu.regs.pc += 3;
}
fn inst_ld_a_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    let val = memory.read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.a = val;
    cpu.regs.pc += 3;
}

fn inst_dec_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using a `sub_?bit` function here, since these instructions
    // (8080 16-bit decrement) don't affect any flags.
    let old_val = cpu.regs.bc;
    let new_val = old_val.wrapping_sub(1);
    cpu.regs.bc = new_val;

    cpu.regs.pc += 1;
}
fn inst_dec_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using a `sub_?bit` function here, since these instructions
    // (8080 16-bit decrement) don't affect any flags.
    let old_val = cpu.regs.de;
    let new_val = old_val.wrapping_sub(1);
    cpu.regs.de = new_val;

    cpu.regs.pc += 1;
}
fn inst_dec_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using a `sub_?bit` function here, since these instructions
    // (8080 16-bit decrement) don't affect any flags.
    let old_val = cpu.regs.hl;
    let new_val = old_val.wrapping_sub(1);
    cpu.regs.hl = new_val;

    cpu.regs.pc += 1;
}
fn inst_dec_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    // We're not using a `sub_?bit` function here, since these instructions
    // (8080 16-bit decrement) don't affect any flags.
    let old_val = cpu.regs.sp;
    let new_val = old_val.wrapping_sub(1);
    cpu.regs.sp = new_val;

    cpu.regs.pc += 1;
}

fn inst_inc_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.bc, result);
    cpu.regs.pc += 1;
}
fn inst_inc_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.de, result);
    cpu.regs.pc += 1;
}
fn inst_inc_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.hl, result);
    cpu.regs.pc += 1;
}
fn inst_inc_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let result = inc_8bit!(cpu.regs.flags, old_val);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_dec_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.bc, result);
    cpu.regs.pc += 1;
}
fn inst_dec_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.de, result);
    cpu.regs.pc += 1;
}
fn inst_dec_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.hl, result);
    cpu.regs.pc += 1;
}
fn inst_dec_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let result = dec_8bit!(cpu.regs.flags, old_val);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_ld_c_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, imm);
    cpu.regs.pc += 2;
}
fn inst_ld_e_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, imm);
    cpu.regs.pc += 2;
}
fn inst_ld_l_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, imm);
    cpu.regs.pc += 2;
}
fn inst_ld_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.a = imm;
    cpu.regs.pc += 2;
}

fn inst_rrca(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };

    cpu.regs.flags.carry = (old_val & 0b0000_0001) != 0;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.add_sub = false;

    cpu.regs.a = new_val;
    cpu.regs.pc += 1;
}

fn inst_rra(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };

    cpu.regs.flags.carry = (old_val & 0b0000_0001) != 0;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.add_sub = false;

    cpu.regs.a = new_val;
    cpu.regs.pc += 1;
}

fn inst_cpl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    cpu.regs.a = !old_val;

    cpu.regs.flags.half_carry = true;
    cpu.regs.flags.add_sub = true;

    cpu.regs.pc += 1;
}

fn inst_ccf(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.flags.half_carry = cpu.regs.flags.carry;
    cpu.regs.flags.carry = !cpu.regs.flags.carry;
    cpu.regs.flags.add_sub = false;

    cpu.regs.pc += 1;
}

// Register operations and Halt (0x40 to 0x7F):
fn inst_ld_b_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_high_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_low_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_high_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_low_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_high_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = get_low_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_b_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_halt(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.halted = true;
}
fn inst_ld_b_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_high_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_d_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_high_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_h_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_high_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_mem_hl_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = cpu.regs.a;
    memory.write_byte(addr, val, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_ld_c_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.bc);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.bc);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.de);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.de);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_high_of_16bit!(cpu.regs.hl);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = get_low_of_16bit!(cpu.regs.hl);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let val = memory.read_byte(addr, cpu.cycle_timestamp);
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}
fn inst_ld_c_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_low_of_16bit!(cpu.regs.bc, val);

    cpu.regs.pc += 1;
}
fn inst_ld_e_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_low_of_16bit!(cpu.regs.de, val);

    cpu.regs.pc += 1;
}
fn inst_ld_l_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    set_low_of_16bit!(cpu.regs.hl, val);

    cpu.regs.pc += 1;
}
fn inst_ld_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    cpu.regs.a = val;

    cpu.regs.pc += 1;
}

// Value manipulaton operations (0x80 to 0xBF):
fn inst_add_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.bc);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.bc);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.bc);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.bc);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.de);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.de);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.de);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.de);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.hl);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.hl);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.hl);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.hl);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_add_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = cpu.regs.a;

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sub_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = cpu.regs.a;

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.bc);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.bc);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.bc);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.bc);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.de);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.de);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.de);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.de);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.hl);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.hl);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.hl);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.hl);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_adc_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = cpu.regs.a;

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_sbc_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = cpu.regs.a;

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_and_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_high_of_16bit!(cpu.regs.bc);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_low_of_16bit!(cpu.regs.bc);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_high_of_16bit!(cpu.regs.de);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_low_of_16bit!(cpu.regs.de);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_high_of_16bit!(cpu.regs.hl);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_low_of_16bit!(cpu.regs.hl);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_and = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_and_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = cpu.regs.a;

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_xor_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_high_of_16bit!(cpu.regs.bc);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_low_of_16bit!(cpu.regs.bc);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_high_of_16bit!(cpu.regs.de);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_low_of_16bit!(cpu.regs.de);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_high_of_16bit!(cpu.regs.hl);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_low_of_16bit!(cpu.regs.hl);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_xor = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_xor_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = cpu.regs.a;

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_or_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_high_of_16bit!(cpu.regs.bc);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_low_of_16bit!(cpu.regs.bc);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_high_of_16bit!(cpu.regs.de);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_low_of_16bit!(cpu.regs.de);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_high_of_16bit!(cpu.regs.hl);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_low_of_16bit!(cpu.regs.hl);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_or = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}
fn inst_or_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = cpu.regs.a;

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 1;
}

fn inst_cp_a_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_high_of_16bit!(cpu.regs.bc);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_low_of_16bit!(cpu.regs.bc);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_high_of_16bit!(cpu.regs.de);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_low_of_16bit!(cpu.regs.de);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_high_of_16bit!(cpu.regs.hl);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_low_of_16bit!(cpu.regs.hl);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let addr = cpu.regs.hl;
    let to_cmp = memory.read_byte(addr, cpu.cycle_timestamp);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}
fn inst_cp_a_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = cpu.regs.a;

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 1;
}

// Calls & Pushes (0xC0 to 0xFF):
fn inst_ret_nz(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.zero {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_nc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.carry {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_po(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    // Note: the `po` up there means `parity odd`, not `parity overflow`.
    if !cpu.regs.flags.parity_overflow {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_p(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.sign {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_pop_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    cpu.regs.bc = new_val;

    cpu.regs.pc += 1;
}
fn inst_pop_de(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    cpu.regs.de = new_val;

    cpu.regs.pc += 1;
}
fn inst_pop_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    cpu.regs.hl = new_val;

    cpu.regs.pc += 1;
}
fn inst_pop_af(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.flags = unpack_flags!(stack_pop_8bit!(cpu.regs, memory, cpu.cycle_timestamp));
    cpu.regs.a = stack_pop_8bit!(cpu.regs, memory, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}

fn inst_jp_nz_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.zero {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_nc_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.carry {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_po_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    // Note: the `po` up there means `parity odd`, not `parity overflow`.
    if !cpu.regs.flags.parity_overflow {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_p_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.sign {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
}

fn inst_out_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let accumulator = cpu.regs.a;
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    let addr = compose_16bit_from_8bit!(accumulator, imm);

    memory.peripheral_write_byte(addr, accumulator, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}

fn inst_ex_mem_sp_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_mem_new_hl = memory.read_word(cpu.regs.sp, cpu.cycle_timestamp);
    let old_hl_new_mem = cpu.regs.hl;

    memory.write_word(cpu.regs.sp, old_hl_new_mem, cpu.cycle_timestamp);
    cpu.regs.hl = old_mem_new_hl;

    cpu.regs.pc += 1;
}

fn inst_di(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.iff1 = false;
    cpu.iff2 = false;

    cpu.regs.pc += 1;
}

fn inst_call_nz_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.zero {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_nc_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.carry {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_po_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    // Note: the `po` up there means `parity odd`, not `parity overflow`.
    if !cpu.regs.flags.parity_overflow {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_p_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if !cpu.regs.flags.sign {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}

fn inst_push_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.bc, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_push_de(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.de, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_push_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.hl, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}
fn inst_push_af(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let accumulator = cpu.regs.a;
    let flags_b = pack_flags!(cpu.regs.flags);

    stack_push_8bit!(cpu.regs, memory, accumulator, cpu.cycle_timestamp);
    stack_push_8bit!(cpu.regs, memory, flags_b, cpu.cycle_timestamp);

    cpu.regs.pc += 1;
}

fn inst_add_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sub_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_and_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_or_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}

fn inst_rst_00h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x00;
}
fn inst_rst_10h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x10;
}
fn inst_rst_20h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x20;
}
fn inst_rst_30h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x30;
}

fn inst_ret_z(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.zero {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.carry {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_pe(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.parity_overflow {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}
fn inst_ret_m(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.sign {
        cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
        cpu.added_delay = 6;
    } else {
        cpu.regs.pc += 1;
    }
}

fn inst_ret(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
}
fn inst_exx(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_bc_new_bc_prime = cpu.regs.bc;
    let old_de_new_de_prime = cpu.regs.de;
    let old_hl_new_hl_prime = cpu.regs.hl;

    let old_bc_prime_new_bc = cpu.regs.bc_prime;
    let old_de_prime_new_de = cpu.regs.de_prime;
    let old_hl_prime_new_hl = cpu.regs.hl_prime;

    cpu.regs.bc = old_bc_prime_new_bc;
    cpu.regs.de = old_de_prime_new_de;
    cpu.regs.hl = old_hl_prime_new_hl;

    cpu.regs.bc_prime = old_bc_new_bc_prime;
    cpu.regs.de_prime = old_de_new_de_prime;
    cpu.regs.hl_prime = old_hl_new_hl_prime;

    cpu.regs.pc += 1;
}
fn inst_jp_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.pc = cpu.regs.hl;
}
fn inst_ld_sp_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.sp = cpu.regs.hl;
    cpu.regs.pc += 1;
}

fn inst_jp_z_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.zero {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_c_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.carry {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_pe_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.parity_overflow {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_jp_m_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.sign {
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
    } else {
        cpu.regs.pc += 3;
    }
}

fn inst_in_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let accumulator = cpu.regs.a;
    let imm = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);
    let addr = compose_16bit_from_8bit!(accumulator, imm);

    cpu.regs.a = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_ex_de_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_de_new_hl = cpu.regs.de;
    let old_hl_new_de = cpu.regs.hl;

    cpu.regs.de = old_hl_new_de;
    cpu.regs.hl = old_de_new_hl;

    cpu.regs.pc += 1;
}
fn inst_ei(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.iff1 = true;
    cpu.iff2 = true;

    cpu.regs.pc += 1;
}

fn inst_call_z_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.zero {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_c_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.carry {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_pe_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.parity_overflow {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_m_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    if cpu.regs.flags.sign {
        stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
        cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
        cpu.added_delay = 7;
    } else {
        cpu.regs.pc += 3;
    }
}
fn inst_call_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 3, cpu.cycle_timestamp);
    cpu.regs.pc = memory.read_word(cpu.regs.pc + 1, cpu.cycle_timestamp);
}

fn inst_adc_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_xor_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_cp_a_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = memory.read_byte(cpu.regs.pc + 1, cpu.cycle_timestamp);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);

    cpu.regs.pc += 2;
}

fn inst_rst_08h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x08;
}
fn inst_rst_18h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x18;
}
fn inst_rst_28h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x28;
}
fn inst_rst_38h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.pc + 1, cpu.cycle_timestamp);
    cpu.regs.pc = 0x38;
}


// Extended instructions:

fn inst_in_b_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_d_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_h_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    cpu.regs.pc += 2;
}

fn inst_out_mem_bc_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.bc);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.de);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.hl);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_0(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = 0u8;
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}

fn inst_sbc_hl_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_sub = cpu.regs.bc;

    let result = sub_16bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_hl_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_sub = cpu.regs.de;

    let result = sub_16bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_hl_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_sub = cpu.regs.hl;

    let result = sub_16bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_hl_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_sub = cpu.regs.sp;

    let result = sub_16bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}

fn inst_ld_mem_im16_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = cpu.regs.bc;

    memory.write_word(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}
fn inst_ld_mem_im16_de(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = cpu.regs.de;

    memory.write_word(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}
fn inst_ld_mem_im16_hl_2(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = cpu.regs.hl;

    memory.write_word(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}
fn inst_ld_mem_im16_sp(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = cpu.regs.sp;

    memory.write_word(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}

fn inst_neg(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let mut new_val_int = 0 - (old_val as i32);

    if new_val_int < 0 {
        cpu.regs.flags.carry = true;
        new_val_int = ((new_val_int as u32) & 0xFF) as i32;
    } else {
        cpu.regs.flags.carry = false;
    }

    let new_val = new_val_int as u8;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.half_carry = (old_val & 0x0F) > 0;
    cpu.regs.flags.parity_overflow = (old_val == 0x80);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_retn(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.iff1 = cpu.iff2;
    cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
}

fn inst_im0(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.im = cpu::InterruptMode::Mode0;
    cpu.regs.pc += 2;
}
fn inst_im1(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.im = cpu::InterruptMode::Mode1;
    cpu.regs.pc += 2;
}

fn inst_ld_i_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.i = cpu.regs.a;
    cpu.regs.pc += 2;
}
fn inst_ld_a_i(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.i;

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = cpu.iff2;

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_rrd(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_mem = memory.read_byte(addr, cpu.cycle_timestamp);
    let old_a = cpu.regs.a;

    let old_a_low = old_a & 0x0F;
    let old_mem_low = old_mem & 0x0F;
    let old_mem_high = old_mem >> 4;

    let new_a = (old_a & 0xF0) | old_mem_low;
    let new_mem = (old_a_low << 4) | old_mem_high;

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_a == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_a);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_a);

    cpu.regs.a = new_a;
    memory.write_byte(addr, new_mem, cpu.cycle_timestamp);

    cpu.regs.pc += 2;
}

fn inst_in_c_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_e_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_l_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_in_a_mem_bc(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.bc;
    let new_val = memory.peripheral_read_byte(addr, cpu.cycle_timestamp);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_val);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_out_mem_bc_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.bc);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.de);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.hl);
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_out_mem_bc_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.a;
    let addr = cpu.regs.bc;

    memory.peripheral_write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}

fn inst_adc_hl_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_add = cpu.regs.bc;

    let result = add_16bit_carry!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_adc_hl_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_add = cpu.regs.de;

    let result = add_16bit_carry!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_adc_hl_hl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_add = cpu.regs.hl;

    let result = add_16bit_carry!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}
fn inst_adc_hl_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.hl;
    let to_add = cpu.regs.sp;

    let result = add_16bit_carry!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.hl = result;
    cpu.regs.pc += 2;
}

fn inst_ld_bc_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.bc = new_val;

    cpu.regs.pc += 4;
}
fn inst_ld_de_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.de = new_val;

    cpu.regs.pc += 4;
}
fn inst_ld_hl_mem_im16_2(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let new_val = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.hl = new_val;

    cpu.regs.pc += 4;
}
fn inst_ld_sp_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    cpu.regs.sp = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}

fn inst_reti(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.pc = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    memory.reti_notify();
}

fn inst_im_0_slash_1(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.log_message("Warning: Interrupt mode set to 0/1, this is undefined.".to_owned());
    cpu.im = cpu::InterruptMode::ModeUndefined;
    cpu.regs.pc += 2;
}
fn inst_im_2(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.im = cpu::InterruptMode::Mode2;
    cpu.regs.pc += 2;
}

fn inst_ld_r_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.r = cpu.regs.a;
    cpu.regs.pc += 2;
}

fn inst_ld_a_r(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.r;

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_val == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_val);
    cpu.regs.flags.parity_overflow = cpu.iff2;

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_rld(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_mem = memory.read_byte(addr, cpu.cycle_timestamp);
    let old_a = cpu.regs.a;

    let old_a_low = old_a & 0x0F;
    let old_mem_low = old_mem & 0x0F;
    let old_mem_high = old_mem >> 4;

    let new_a = (old_a & 0xF0) | old_mem_high;
    let new_mem = (old_mem_low << 4) | old_a_low;

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.zero = (new_a == 0);
    cpu.regs.flags.sign = is_8bit_negative!(new_a);
    cpu.regs.flags.parity_overflow = even_parity_8bit!(new_a);

    cpu.regs.a = new_a;
    memory.write_byte(addr, new_mem, cpu.cycle_timestamp);

    cpu.regs.pc += 2;
}

fn inst_ldi(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_de = cpu.regs.de;
    let old_hl = cpu.regs.hl;

    let mem_old_hl = memory.read_byte(old_hl, cpu.cycle_timestamp);
    memory.write_byte(old_de, mem_old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_de = old_de.wrapping_add(1);
    let new_hl = old_hl.wrapping_add(1);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.parity_overflow = (new_bc != 0);

    cpu.regs.bc = new_bc;
    cpu.regs.de = new_de;
    cpu.regs.hl = new_hl;

    cpu.regs.pc += 2;
}
fn inst_ldir(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_de = cpu.regs.de;
    let old_hl = cpu.regs.hl;

    let mem_old_hl = memory.read_byte(old_hl, cpu.cycle_timestamp);
    memory.write_byte(old_de, mem_old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_de = old_de.wrapping_add(1);
    let new_hl = old_hl.wrapping_add(1);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.parity_overflow = false;

    cpu.regs.bc = new_bc;
    cpu.regs.de = new_de;
    cpu.regs.hl = new_hl;

    if new_bc == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_cpi(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_hl = cpu.regs.hl;
    let comparee = cpu.regs.a;
    let to_cmp = memory.read_byte(old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_hl = old_hl.wrapping_add(1);

    let dest_int:   i32 = comparee as i32;
    let to_cmp_int: i32 = to_cmp as i32;

    let mut result_int = dest_int - to_cmp_int;
    if result_int < 0 {
        result_int = ((result_int as u32) & 0xFF) as i32;
    }

    cpu.regs.flags.zero = (result_int == 0);
    cpu.regs.flags.sign = is_8bit_negative!(result_int);
    cpu.regs.flags.half_carry = ((dest_int & 0x0F) - (to_cmp_int & 0x0F)) < 0;
    cpu.regs.flags.parity_overflow = (new_bc != 0);
    cpu.regs.flags.add_sub = true;

    cpu.regs.bc = new_bc;
    cpu.regs.hl = new_hl;

    cpu.regs.pc += 2;
}
fn inst_cpir(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_hl = cpu.regs.hl;
    let comparee = cpu.regs.a;
    let to_cmp = memory.read_byte(old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_hl = old_hl.wrapping_add(1);

    let dest_int:   i32 = comparee as i32;
    let to_cmp_int: i32 = to_cmp as i32;

    let mut result_int = dest_int - to_cmp_int;
    if result_int < 0 {
        result_int = ((result_int as u32) & 0xFF) as i32;
    }

    cpu.regs.flags.zero = (result_int == 0);
    cpu.regs.flags.sign = is_8bit_negative!(result_int);
    cpu.regs.flags.half_carry = ((dest_int & 0x0F) - (to_cmp_int & 0x0F)) < 0;
    cpu.regs.flags.parity_overflow = (new_bc != 0);
    cpu.regs.flags.add_sub = true;

    cpu.regs.bc = new_bc;
    cpu.regs.hl = new_hl;

    if (new_bc == 0) || (result_int == 0) {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_ldd(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_de = cpu.regs.de;
    let old_hl = cpu.regs.hl;

    let mem_old_hl = memory.read_byte(old_hl, cpu.cycle_timestamp);
    memory.write_byte(old_de, mem_old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_de = old_de.wrapping_sub(1);
    let new_hl = old_hl.wrapping_sub(1);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.half_carry = false;
    cpu.regs.flags.parity_overflow = (new_bc != 0);

    cpu.regs.bc = new_bc;
    cpu.regs.de = new_de;
    cpu.regs.hl = new_hl;

    cpu.regs.pc += 2;
}
fn inst_lddr(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_de = cpu.regs.de;
    let old_hl = cpu.regs.hl;

    let mem_old_hl = memory.read_byte(old_hl, cpu.cycle_timestamp);
    memory.write_byte(old_de, mem_old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_de = old_de.wrapping_sub(1);
    let new_hl = old_hl.wrapping_sub(1);

    cpu.regs.flags.add_sub = false;
    cpu.regs.flags.carry = false;
    cpu.regs.flags.parity_overflow = false;

    cpu.regs.bc = new_bc;
    cpu.regs.de = new_de;
    cpu.regs.hl = new_hl;

    if new_bc == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_cpd(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_hl = cpu.regs.hl;
    let comparee = cpu.regs.a;
    let to_cmp = memory.read_byte(old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_hl = old_hl.wrapping_sub(1);

    let dest_int:   i32 = comparee as i32;
    let to_cmp_int: i32 = to_cmp as i32;

    let mut result_int = dest_int - to_cmp_int;
    if result_int < 0 {
        result_int = ((result_int as u32) & 0xFF) as i32;
    }

    cpu.regs.flags.zero = (result_int == 0);
    cpu.regs.flags.sign = is_8bit_negative!(result_int);
    cpu.regs.flags.half_carry = ((dest_int & 0x0F) - (to_cmp_int & 0x0F)) < 0;
    cpu.regs.flags.parity_overflow = (new_bc != 0);
    cpu.regs.flags.add_sub = true;

    cpu.regs.bc = new_bc;
    cpu.regs.hl = new_hl;

    cpu.regs.pc += 2;
}
fn inst_cpdr(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_bc = cpu.regs.bc;
    let old_hl = cpu.regs.hl;
    let comparee = cpu.regs.a;
    let to_cmp = memory.read_byte(old_hl, cpu.cycle_timestamp);

    let new_bc = old_bc.wrapping_sub(1);
    let new_hl = old_hl.wrapping_sub(1);

    let dest_int:   i32 = comparee as i32;
    let to_cmp_int: i32 = to_cmp as i32;

    let mut result_int = dest_int - to_cmp_int;
    if result_int < 0 {
        result_int = ((result_int as u32) & 0xFF) as i32;
    }

    cpu.regs.flags.zero = (result_int == 0);
    cpu.regs.flags.sign = is_8bit_negative!(result_int);
    cpu.regs.flags.half_carry = ((dest_int & 0x0F) - (to_cmp_int & 0x0F)) < 0;
    cpu.regs.flags.parity_overflow = (new_bc != 0);
    cpu.regs.flags.add_sub = true;

    cpu.regs.bc = new_bc;
    cpu.regs.hl = new_hl;

    if (new_bc == 0) || (result_int == 0) {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_ini(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);

    let new_byte_counter = byte_counter.wrapping_sub(1);
    let new_mem_addr = mem_addr.wrapping_add(1);

    let new_val = memory.peripheral_read_byte(io_addr, cpu.cycle_timestamp);
    memory.write_byte(mem_addr, new_val, cpu.cycle_timestamp);

    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = (new_byte_counter == 0);

    cpu.regs.pc += 2;
}
fn inst_inir(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);

    let new_byte_counter = byte_counter.wrapping_sub(1);
    let new_mem_addr = mem_addr.wrapping_add(1);

    let new_val = memory.peripheral_read_byte(io_addr, cpu.cycle_timestamp);
    memory.write_byte(mem_addr, new_val, cpu.cycle_timestamp);

    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = true;

    if new_byte_counter == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_outi(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);
    let new_byte_counter = byte_counter.wrapping_sub(1);
    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);

    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;

    let new_val = memory.read_byte(mem_addr, cpu.cycle_timestamp);
    memory.peripheral_write_byte(io_addr, new_val, cpu.cycle_timestamp);

    let new_mem_addr = mem_addr.wrapping_add(1);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = (new_byte_counter == 0);

    cpu.regs.pc += 2;
}
fn inst_outir(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);
    let new_byte_counter = byte_counter.wrapping_sub(1);
    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);

    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;

    let new_val = memory.read_byte(mem_addr, cpu.cycle_timestamp);
    memory.peripheral_write_byte(io_addr, new_val, cpu.cycle_timestamp);

    let new_mem_addr = mem_addr.wrapping_add(1);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = true;

    if new_byte_counter == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}
fn inst_ind(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);

    let new_byte_counter = byte_counter.wrapping_sub(1);
    let new_mem_addr = mem_addr.wrapping_sub(1);

    let new_val = memory.peripheral_read_byte(io_addr, cpu.cycle_timestamp);
    memory.write_byte(mem_addr, new_val, cpu.cycle_timestamp);

    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = (new_byte_counter == 0);

    cpu.regs.pc += 2;
}
fn inst_indr(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);

    let new_byte_counter = byte_counter.wrapping_sub(1);
    let new_mem_addr = mem_addr.wrapping_sub(1);

    let new_val = memory.peripheral_read_byte(io_addr, cpu.cycle_timestamp);
    memory.write_byte(mem_addr, new_val, cpu.cycle_timestamp);

    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = true;

    if new_byte_counter == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

fn inst_outd(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);
    let new_byte_counter = byte_counter.wrapping_sub(1);
    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);

    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;

    let new_val = memory.read_byte(mem_addr, cpu.cycle_timestamp);
    memory.peripheral_write_byte(io_addr, new_val, cpu.cycle_timestamp);

    let new_mem_addr = mem_addr.wrapping_sub(1);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = (new_byte_counter == 0);

    cpu.regs.pc += 2;
}
fn inst_outdr(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let byte_counter = get_high_of_16bit!(cpu.regs.bc);
    let new_byte_counter = byte_counter.wrapping_sub(1);
    set_high_of_16bit!(cpu.regs.bc, new_byte_counter);

    let io_addr = cpu.regs.bc;
    let mem_addr = cpu.regs.hl;

    let new_val = memory.read_byte(mem_addr, cpu.cycle_timestamp);
    memory.peripheral_write_byte(io_addr, new_val, cpu.cycle_timestamp);

    let new_mem_addr = mem_addr.wrapping_sub(1);
    cpu.regs.hl = new_mem_addr;

    cpu.regs.flags.add_sub = true;
    cpu.regs.flags.zero = true;

    if new_byte_counter == 0 {
        cpu.regs.pc += 2;
    } else {
        cpu.added_delay = 5;
    }
}

// Main bit instructions (CB-Prefixed):
fn inst_rlc_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rlc_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rrc_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_rl_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rl_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_rr_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_sla_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val << 1;
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val << 1;
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val << 1;
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val << 1;
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val << 1;
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val << 1;
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sla_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val << 1;
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_sra_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sra_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_sll_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = (old_val << 1) | 0b0000_0001;
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_sll_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = (old_val << 1) | 0b0000_0001;
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_srl_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val >> 1;
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val >> 1;
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val >> 1;
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val >> 1;
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val >> 1;
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val >> 1;
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}
fn inst_srl_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val >> 1;
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 2;
}

fn inst_bit_0_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}
fn inst_bit_0_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 2;
}

fn inst_bit_1_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}
fn inst_bit_1_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 2;
}

fn inst_bit_2_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}
fn inst_bit_2_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 2;
}

fn inst_bit_3_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}
fn inst_bit_3_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 2;
}

fn inst_bit_4_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}
fn inst_bit_4_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 2;
}

fn inst_bit_5_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}
fn inst_bit_5_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 2;
}

fn inst_bit_6_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}
fn inst_bit_6_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 2;
}

fn inst_bit_7_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.bc);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.de);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_high_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = get_low_of_16bit!(cpu.regs.hl);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}
fn inst_bit_7_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let value = cpu.regs.a;

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 2;
}

fn inst_res_0_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 0);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 0);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 0);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 0);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 0);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 0);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_0_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_0_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 0);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_1_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 1);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 1);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 1);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 1);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 1);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 1);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_1_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_1_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 1);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_2_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 2);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 2);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 2);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 2);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 2);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 2);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_2_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_2_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 2);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_3_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 3);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 3);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 3);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 3);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 3);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 3);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_3_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_3_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 3);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_4_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 4);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 4);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 4);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 4);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 4);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 4);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_4_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_4_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 4);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_5_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 5);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 5);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 5);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 5);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 5);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 5);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_5_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_5_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 5);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_6_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 6);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 6);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 6);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 6);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 6);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 6);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_6_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_6_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 6);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_res_7_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 7);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val & !(1 << 7);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 7);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val & !(1 << 7);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 7);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val & !(1 << 7);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_res_7_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_res_7_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val & !(1 << 7);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_0_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 0);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 0);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 0);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 0);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 0);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 0);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_0_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_0_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 0);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_1_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 1);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 1);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 1);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 1);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 1);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 1);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_1_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_1_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 1);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_2_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 2);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 2);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 2);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 2);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 2);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 2);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_2_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_2_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 2);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_3_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 3);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 3);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 3);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 3);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 3);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 3);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_3_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_3_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 3);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_4_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 4);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 4);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 4);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 4);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 4);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 4);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_4_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_4_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 4);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_5_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 5);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 5);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 5);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 5);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 5);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 5);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_5_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_5_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 5);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_6_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 6);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 6);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 6);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 6);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 6);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 6);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_6_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_6_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 6);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

fn inst_set_7_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 7);

    set_high_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.bc);
    let new_val = old_val | (1 << 7);

    set_low_of_16bit!(cpu.regs.bc, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 7);

    set_high_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.de);
    let new_val = old_val | (1 << 7);

    set_low_of_16bit!(cpu.regs.de, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_h(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 7);

    set_high_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_l(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.hl);
    let new_val = old_val | (1 << 7);

    set_low_of_16bit!(cpu.regs.hl, new_val);
    cpu.regs.pc += 2;
}
fn inst_set_7_mem_hl(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = cpu.regs.hl;
    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);

    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_set_7_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let new_val = old_val | (1 << 7);

    cpu.regs.a = new_val;
    cpu.regs.pc += 2;
}

// IX instructions (DD-Prefixed):
fn inst_add_ix_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    let to_add = cpu.regs.bc;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.ix = result;
    cpu.regs.pc += 2;
}
fn inst_add_ix_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    let to_add = cpu.regs.de;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.ix = result;
    cpu.regs.pc += 2;
}
fn inst_add_ix_ix(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    let to_add = cpu.regs.ix;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.ix = result;
    cpu.regs.pc += 2;
}
fn inst_add_ix_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    let to_add = cpu.regs.sp;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.ix = result;
    cpu.regs.pc += 2;
}
fn inst_ld_ix_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.ix = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}
fn inst_ld_mem_im16_ix(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    memory.write_word(addr, cpu.regs.ix, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_inc_ix(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    cpu.regs.ix = old_val.wrapping_add(1);

    cpu.regs.pc += 2;
}
fn inst_inc_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.ix);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.ix, result);
    cpu.regs.pc += 2;
}
fn inst_dec_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.ix);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.ix, result);
    cpu.regs.pc += 2;
}
fn inst_ld_ixh_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_ix_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    cpu.regs.ix = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_dec_ix(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.ix;
    cpu.regs.ix = old_val.wrapping_sub(1);

    cpu.regs.pc += 2;
}
fn inst_inc_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.ix);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.ix, result);
    cpu.regs.pc += 2;
}
fn inst_dec_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.ix);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.ix, result);
    cpu.regs.pc += 2;
}
fn inst_ld_ixl_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 3;
}

fn inst_ld_b_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_b_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_c_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_c_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_d_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_d_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_e_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_e_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    cpu.regs.a = new_val;

    cpu.regs.pc += 2;
}
fn inst_ld_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    cpu.regs.a = new_val;

    cpu.regs.pc += 2;
}

fn inst_ld_ixh_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.a;
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.a;
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixh_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_high_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_ixl_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.ix);
    set_low_of_16bit!(cpu.regs.ix, new_val);

    cpu.regs.pc += 2;
}

fn inst_add_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.ix);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_add_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.ix);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sub_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.ix);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sub_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.ix);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}

fn inst_adc_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.ix);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_adc_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.ix);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.ix);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.ix);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_and_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_high_of_16bit!(cpu.regs.ix);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_and_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_low_of_16bit!(cpu.regs.ix);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_or_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_high_of_16bit!(cpu.regs.ix);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_or_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_low_of_16bit!(cpu.regs.ix);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_xor_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_high_of_16bit!(cpu.regs.ix);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_xor_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_low_of_16bit!(cpu.regs.ix);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_cp_a_ixh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_high_of_16bit!(cpu.regs.ix);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);
    cpu.regs.pc += 2;
}
fn inst_cp_a_ixl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_low_of_16bit!(cpu.regs.ix);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);
    cpu.regs.pc += 2;
}

fn inst_pop_ix(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.ix = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_push_ix(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.ix, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_ex_mem_sp_ix(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_mem_sp_new_ix = memory.read_word(cpu.regs.sp, cpu.cycle_timestamp);
    let old_ix_new_mem_sp = cpu.regs.ix;

    cpu.regs.ix = old_mem_sp_new_ix;
    memory.write_word(cpu.regs.sp, old_ix_new_mem_sp, cpu.cycle_timestamp);

    cpu.regs.pc += 2;
}
fn inst_jp_ix(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.pc = cpu.regs.ix;
}
fn inst_ld_sp_ix(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.sp = cpu.regs.ix;
    cpu.regs.pc += 2;
}

fn inst_inc_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(addr, result, cpu.cycle_timestamp);
    cpu.regs.pc += 3;
}
fn inst_dec_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(addr, result, cpu.cycle_timestamp);
    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(cpu.regs.pc + 3, cpu.cycle_timestamp);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_ld_b_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_c_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_d_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_e_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_h_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_l_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 3;
}

fn inst_ld_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_high_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_low_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_high_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_low_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_high_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = get_low_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let new_val = cpu.regs.a;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_add_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_adc_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_sub_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_sbc_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_and_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_and = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_or_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_or = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_xor_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_xor = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_cp_a_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let _ = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);
    cpu.regs.pc += 3;
}

// IX bit instructions (DDCB-Prefixed):
fn inst_rlc_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rrc_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rl_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rr_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sla_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sra_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sll_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_srl_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_bit_0_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 4;
}
fn inst_bit_1_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 4;
}
fn inst_bit_2_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 4;
}
fn inst_bit_3_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 4;
}
fn inst_bit_4_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 4;
}
fn inst_bit_5_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 4;
}
fn inst_bit_6_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 4;
}
fn inst_bit_7_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 4;
}

fn inst_res_0_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_1_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_2_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_3_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_4_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_5_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_6_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_7_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_0_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_1_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_2_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_3_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_4_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_5_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_6_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_7_mem_ix_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_ix_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.ix, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

// IY instructions (FD-Prefixed):
fn inst_add_iy_bc(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    let to_add = cpu.regs.bc;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.iy = result;
    cpu.regs.pc += 2;
}
fn inst_add_iy_de(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    let to_add = cpu.regs.de;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.iy = result;
    cpu.regs.pc += 2;
}
fn inst_add_iy_iy(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    let to_add = cpu.regs.iy;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.iy = result;
    cpu.regs.pc += 2;
}
fn inst_add_iy_sp(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    let to_add = cpu.regs.sp;
    let result = add_16bit!(cpu.regs.flags, old_val, to_add);

    cpu.regs.iy = result;
    cpu.regs.pc += 2;
}
fn inst_ld_iy_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.iy = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    cpu.regs.pc += 4;
}
fn inst_ld_mem_im16_iy(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    memory.write_word(addr, cpu.regs.iy, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_inc_iy(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    cpu.regs.iy = old_val.wrapping_add(1);

    cpu.regs.pc += 2;
}
fn inst_inc_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.iy);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.iy, result);
    cpu.regs.pc += 2;
}
fn inst_dec_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_high_of_16bit!(cpu.regs.iy);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_high_of_16bit!(cpu.regs.iy, result);
    cpu.regs.pc += 2;
}
fn inst_ld_iyh_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_iy_mem_im16(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let addr = memory.read_word(cpu.regs.pc + 2, cpu.cycle_timestamp);
    cpu.regs.iy = memory.read_word(addr, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_dec_iy(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.iy;
    cpu.regs.iy = old_val.wrapping_sub(1);

    cpu.regs.pc += 2;
}
fn inst_inc_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.iy);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.iy, result);
    cpu.regs.pc += 2;
}
fn inst_dec_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = get_low_of_16bit!(cpu.regs.iy);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    set_low_of_16bit!(cpu.regs.iy, result);
    cpu.regs.pc += 2;
}
fn inst_ld_iyl_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let new_val = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 3;
}

fn inst_ld_b_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_b_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_c_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_c_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_d_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_d_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_e_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_e_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    cpu.regs.a = new_val;

    cpu.regs.pc += 2;
}
fn inst_ld_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    cpu.regs.a = new_val;

    cpu.regs.pc += 2;
}

fn inst_ld_iyh_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_b(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.bc);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_c(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.bc);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_d(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.de);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_e(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.de);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.a;
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_a(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = cpu.regs.a;
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyh_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_high_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_high_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}
fn inst_ld_iyl_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let new_val = get_low_of_16bit!(cpu.regs.iy);
    set_low_of_16bit!(cpu.regs.iy, new_val);

    cpu.regs.pc += 2;
}

fn inst_add_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.iy);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_add_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.iy);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sub_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.iy);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sub_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.iy);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}

fn inst_adc_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_high_of_16bit!(cpu.regs.iy);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_adc_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_add = get_low_of_16bit!(cpu.regs.iy);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_high_of_16bit!(cpu.regs.iy);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_sbc_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_sub = get_low_of_16bit!(cpu.regs.iy);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_and_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_high_of_16bit!(cpu.regs.iy);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_and_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_and = get_low_of_16bit!(cpu.regs.iy);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_or_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_high_of_16bit!(cpu.regs.iy);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_or_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_or = get_low_of_16bit!(cpu.regs.iy);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_xor_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_high_of_16bit!(cpu.regs.iy);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_xor_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let old_val = cpu.regs.a;
    let to_xor = get_low_of_16bit!(cpu.regs.iy);

    let result = xor_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 2;
}
fn inst_cp_a_iyh(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_high_of_16bit!(cpu.regs.iy);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);
    cpu.regs.pc += 2;
}
fn inst_cp_a_iyl(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    let val = cpu.regs.a;
    let to_cmp = get_low_of_16bit!(cpu.regs.iy);

    let _ = sub_8bit!(cpu.regs.flags, val, to_cmp, false);
    cpu.regs.pc += 2;
}

fn inst_pop_iy(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    cpu.regs.iy = stack_pop_16bit!(cpu.regs, memory, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_push_iy(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    stack_push_16bit!(cpu.regs, memory, cpu.regs.iy, cpu.cycle_timestamp);
    cpu.regs.pc += 2;
}
fn inst_ex_mem_sp_iy(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let old_mem_sp_new_iy = memory.read_word(cpu.regs.sp, cpu.cycle_timestamp);
    let old_iy_new_mem_sp = cpu.regs.iy;

    cpu.regs.iy = old_mem_sp_new_iy;
    memory.write_word(cpu.regs.sp, old_iy_new_mem_sp, cpu.cycle_timestamp);

    cpu.regs.pc += 2;
}
fn inst_jp_iy(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.pc = cpu.regs.iy;
}
fn inst_ld_sp_iy(cpu: &mut cpu::CPU, _memory: &mut memory::MemorySystem) {
    cpu.regs.sp = cpu.regs.iy;
    cpu.regs.pc += 2;
}

fn inst_inc_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let result = inc_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(addr, result, cpu.cycle_timestamp);
    cpu.regs.pc += 3;
}
fn inst_dec_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let result = dec_8bit!(cpu.regs.flags, old_val);

    memory.write_byte(addr, result, cpu.cycle_timestamp);
    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(cpu.regs.pc + 3, cpu.cycle_timestamp);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_ld_b_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_c_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_d_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_e_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_h_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_l_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 3;
}
fn inst_ld_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = memory.read_byte(addr, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 3;
}

fn inst_ld_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_high_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_low_of_16bit!(cpu.regs.bc);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_high_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_low_of_16bit!(cpu.regs.de);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_high_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = get_low_of_16bit!(cpu.regs.hl);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_ld_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let new_val = cpu.regs.a;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 3;
}
fn inst_add_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, false);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_adc_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_add = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = add_8bit!(cpu.regs.flags, old_val, to_add, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_sub_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_sbc_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = sub_8bit!(cpu.regs.flags, old_val, to_sub, cpu.regs.flags.carry);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_and_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_and = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = and_8bit!(cpu.regs.flags, old_val, to_and);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_or_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_or = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_or);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_xor_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_xor = memory.read_byte(addr, cpu.cycle_timestamp);

    let result = or_8bit!(cpu.regs.flags, old_val, to_xor);

    cpu.regs.a = result;
    cpu.regs.pc += 3;
}
fn inst_cp_a_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = cpu.regs.a;
    let to_sub = memory.read_byte(addr, cpu.cycle_timestamp);

    let _ = sub_8bit!(cpu.regs.flags, old_val, to_sub, false);
    cpu.regs.pc += 3;
}

// IY bit instructions (FDCB-Prefixed):
fn inst_rlc_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rlc_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | if (old_val & 0b1000_0000) != 0 { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rrc_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rrc_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | if (old_val & 0b0000_0001) != 0 { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rl_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rl_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val << 1) | if cpu.regs.flags.carry { 0b0000_0001 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_rr_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_rr_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);

    let new_val = (old_val >> 1) | if cpu.regs.flags.carry { 0b1000_0000 } else { 0 };
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sla_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sla_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val << 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sra_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sra_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val >> 1) | (old_val & 0b1000_0000);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_sll_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_sll_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = (old_val << 1) | 0b0000_0001;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_left_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_srl_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}
fn inst_srl_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val >> 1;
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    apply_shift_right_flags!(cpu.regs.flags, old_val, new_val);
    cpu.regs.pc += 4;
}

fn inst_bit_0_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 0, value);
    cpu.regs.pc += 4;
}
fn inst_bit_1_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 1, value);
    cpu.regs.pc += 4;
}
fn inst_bit_2_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 2, value);
    cpu.regs.pc += 4;
}
fn inst_bit_3_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 3, value);
    cpu.regs.pc += 4;
}
fn inst_bit_4_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 4, value);
    cpu.regs.pc += 4;
}
fn inst_bit_5_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 5, value);
    cpu.regs.pc += 4;
}
fn inst_bit_6_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 6, value);
    cpu.regs.pc += 4;
}
fn inst_bit_7_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let value = memory.read_byte(addr, cpu.cycle_timestamp);

    apply_inst_bit_flags!(cpu.regs.flags, 7, value);
    cpu.regs.pc += 4;
}

fn inst_res_0_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_0_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_1_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_1_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_2_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_2_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_3_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_3_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_4_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_4_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_5_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_5_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_6_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_6_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_res_7_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_res_7_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val & !(1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_0_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_0_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 0);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_1_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_1_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 1);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_2_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_2_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 2);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_3_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_3_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 3);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_4_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_4_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 4);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_5_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_5_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 5);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_6_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_6_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 6);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}

fn inst_set_7_mem_iy_im8_b(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_c(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.bc, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_d(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_e(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.de, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_h(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_high_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_l(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    set_low_of_16bit!(cpu.regs.hl, new_val);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);

    cpu.regs.pc += 4;
}
fn inst_set_7_mem_iy_im8_a(cpu: &mut cpu::CPU, memory: &mut memory::MemorySystem) {
    let offset = memory.read_byte(cpu.regs.pc + 2, cpu.cycle_timestamp);
    let addr = add_16bit_signed_8bit!(cpu.regs.iy, offset);

    let old_val = memory.read_byte(addr, cpu.cycle_timestamp);
    let new_val = old_val | (1 << 7);
    memory.write_byte(addr, new_val, cpu.cycle_timestamp);
    cpu.regs.a = new_val;

    cpu.regs.pc += 4;
}
