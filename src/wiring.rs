//! SWD bit-bang pins and the UART that the USB serial bridge talks to.

use embassy_stm32::gpio::{Flex, Level, Output, Speed};
use embassy_stm32::usart::{self, BufferedUart, Config, DataBits, Parity, StopBits};
use embassy_stm32::{Peripherals, bind_interrupts, can::Can, peripherals};
use static_cell::StaticCell;

use crate::swd::Swd;

pub struct Ports {
    pub can: Can<'static>,
    pub uart: Option<BufferedUart<'static>>,
}

// UART7's interrupt has to call the buffered driver's handler.
bind_interrupts!(struct UartIrqs {
    UART7 => usart::BufferedInterruptHandler<peripherals::UART7>;
});

// These outlive `configure`, so they cannot be ordinary local variables.
static TX_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static RX_BUF: StaticCell<[u8; 64]> = StaticCell::new();

pub fn configure(p: Peripherals) -> (crate::board::Usb, Ports, Option<Swd>) {
    let usb = crate::board::usb(p.USB_OTG_HS, p.PA12, p.PA11);

    // P11 servo header, signal pins only (the middle pin of each group is 5 V).
    // SWCLK = PA0 = TIM2_CH1, schematic signal pin 1.
    // SWDIO = PA2 = TIM2_CH3, schematic signal pin 2.
    let swclk = Output::new(p.PA0, Level::Low, Speed::Low);
    let swdio = Flex::new(p.PA2);
    let swd = Some(Swd::new(swclk, swdio));

    // P13 UART7: pin 3 = RX = PE7, pin 4 = TX = PE8. 115200 baud, 8 data bits, no parity, 1 stop bit.
    let mut uart_config = Config::default();
    uart_config.baudrate = 115_200;
    uart_config.data_bits = DataBits::DataBits8;
    uart_config.parity = Parity::ParityNone;
    uart_config.stop_bits = StopBits::STOP1;
    // Wake the USB bridge as soon as one byte arrives, not only when the buffer is half full.
    uart_config.eager_reads = Some(1);
    let uart = BufferedUart::new(
        p.UART7,
        p.PE7,
        p.PE8,
        TX_BUF.init([0; 64]),
        RX_BUF.init([0; 64]),
        UartIrqs,
        uart_config,
    )
    .unwrap();

    let can = crate::board::can1(p.FDCAN1, p.PD0, p.PD1);
    (
        usb,
        Ports {
            can,
            uart: Some(uart),
        },
        swd,
    )
}
