// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Copyright (c) 2023 Amazon.com, Inc. or its affiliates.
//
// Author(s):
//   - Ugur Usug <ugurus@amazon.com>

//! Pointer Authentication Key A for Code High - EL1
//!
//! Holds bits[127:64] of key used for generic pointer authentication code.

use tock_registers::interfaces::{Readable, Writeable};

pub struct Reg;

impl Readable for Reg {
    type T = u64;
    type R = ();

    sys_coproc_read_raw!(u64, "APGAKeyHi_EL1", "x");
}

impl Writeable for Reg {
    type T = u64;
    type R = ();

    sys_coproc_write_raw!(u64, "APGAKeyHi_EL1", "x");
}

pub const APGAKEYHI_EL1: Reg = Reg {};
