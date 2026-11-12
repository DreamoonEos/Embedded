//! Minimal ADIv5 SWJ-DP DPIDR read. The teaching probe does not write memory.

use embassy_stm32::gpio::{Flex, Level, Output, Pull, Speed};

pub struct Swd {
    clock: Output<'static>,
    data: Flex<'static>,
}

impl Swd {
    // Called by students after selecting the two GPIOs.
    pub fn new(mut clock: Output<'static>, mut data: Flex<'static>) -> Self {
        clock.set_low();
        data.set_as_input(Pull::Up);
        Self { clock, data }
    }

    /// Slow GPIO clock for short wires. USB servicing can stretch the interval.
    fn clock(&mut self) -> bool {
        cortex_m::asm::delay(128);
        // The target can change SWDIO on the rising edge. Sample the bit
        // it drove on the previous edge while SWCLK is still low.
        let sampled = self.data.is_high();
        self.clock.set_high();
        cortex_m::asm::delay(128);
        self.clock.set_low();
        sampled
    }

    fn write_bits(&mut self, word: u32, len: u8) {
        for bit in 0..len {
            self.data.set_level(Level::from(word & (1 << bit) != 0));
            self.clock();
        }
    }

    fn read_bits(&mut self, len: u8) -> u32 {
        let mut value = 0;
        for bit in 0..len {
            value |= u32::from(self.clock()) << bit;
        }
        value
    }

    pub fn read_dpidr(&mut self) -> Option<u32> {
        self.data.set_high();
        self.data.set_as_output(Speed::Low);
        self.write_bits(u32::MAX, 32);
        self.write_bits(u32::MAX, 24);
        self.write_bits(0xe79e, 16); // SWJ-DP: JTAG to SWD, LSB first.
        self.write_bits(u32::MAX, 32);
        self.write_bits(u32::MAX, 24);
        self.write_bits(0, 8);

        self.write_bits(0xa5, 8); // Read DP register address 0: DPIDR.
        self.data.set_as_input(Pull::Up);
        self.clock(); // Host-to-target turnaround.
        let ack = self.read_bits(3);
        let value = match ack {
            1 => {
                let value = self.read_bits(32);
                let parity = self.read_bits(1);
                (parity == value.count_ones() % 2).then_some(value)
            }
            2 | 4 => None, // WAIT or FAULT.
            _ => {
                // Lost framing. Keep SWDIO released for any pending data.
                self.read_bits(32);
                self.clock();
                self.clock();
                return None;
            }
        };
        self.clock(); // Target-to-host turnaround.
        self.data.set_low();
        self.data.set_as_output(Speed::Low);
        self.write_bits(0, 8);
        self.data.set_as_input(Pull::Up);
        value
    }
}
