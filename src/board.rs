use core::num::{NonZeroU8, NonZeroU16};

use embassy_stm32::{
    Config, Peri, Peripherals, bind_interrupts,
    can::{
        Can, CanConfigurator, IT0InterruptHandler, IT1InterruptHandler,
        config::{FrameTransmissionConfig, NominalBitTiming},
    },
    peripherals,
    rcc::{
        Hse, HseMode, Hsi48Config, Pll, PllDiv, PllMul, PllPreDiv, PllSource,
        mux::{Fdcansel, Usbsel},
    },
    time::Hertz,
    usb::{self, Driver},
};
use static_cell::StaticCell;

pub type Usb = Driver<'static, peripherals::USB_OTG_HS>;

bind_interrupts!(struct UsbIrqs {
    OTG_HS => usb::InterruptHandler<peripherals::USB_OTG_HS>;
});

bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => IT0InterruptHandler<peripherals::FDCAN1>;
    FDCAN1_IT1 => IT1InterruptHandler<peripherals::FDCAN1>;
});

/// The clock tree is provided. Keep SYSCLK at its default HSI setting.
pub fn init() -> Peripherals {
    let mut config = Config::default();
    config.rcc.hse = Some(Hse {
        freq: Hertz(24_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll2 = Some(Pll {
        source: PllSource::HSE,
        prediv: PllPreDiv::DIV12,
        mul: PllMul::MUL240,
        divp: Some(PllDiv::DIV2),
        divq: Some(PllDiv::DIV6),
        divr: Some(PllDiv::DIV2),
    });
    config.rcc.mux.fdcansel = Fdcansel::PLL2_Q;
    config.rcc.hsi48 = Some(Hsi48Config {
        sync_from_usb: true,
    });
    config.rcc.mux.usbsel = Usbsel::HSI48;
    embassy_stm32::init(config)
}

pub fn usb(
    otg: embassy_stm32::Peri<'static, peripherals::USB_OTG_HS>,
    dp: embassy_stm32::Peri<'static, peripherals::PA12>,
    dm: embassy_stm32::Peri<'static, peripherals::PA11>,
) -> Usb {
    static OUT: StaticCell<[u8; 512]> = StaticCell::new();
    let mut config = usb::Config::default();
    config.vbus_detection = false;
    Driver::new_fs(otg, UsbIrqs, dp, dm, OUT.init([0; 512]), config)
}

/// Provided CAN1 setup for the onboard transceiver and 80 MHz FDCAN clock.
pub fn can1(
    fdcan: Peri<'static, peripherals::FDCAN1>,
    rx: Peri<'static, peripherals::PD0>,
    tx: Peri<'static, peripherals::PD1>,
) -> Can<'static> {
    let mut can = CanConfigurator::new(fdcan, rx, tx, CanIrqs);
    // 1 Mbit/s, 75% sample point. Segment lengths are in time quanta.
    let timing = NominalBitTiming {
        prescaler: NonZeroU16::new(1).unwrap(),
        seg1: NonZeroU8::new(59).unwrap(),
        seg2: NonZeroU8::new(20).unwrap(),
        sync_jump_width: NonZeroU8::new(20).unwrap(),
    };
    let config = can
        .config()
        .set_nominal_bit_timing(timing)
        .set_frame_transmit(FrameTransmissionConfig::ClassicCanOnly);
    can.set_config(config);
    can.into_normal_mode()
}
