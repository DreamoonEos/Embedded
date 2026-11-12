#![no_std]
#![no_main]

mod board;
mod dap_usb;
mod protocol;
pub mod swd;
mod wiring;

use board::Usb;
use embassy_executor::Spawner;
use embassy_futures::{join::join4, select::select};
use embassy_stm32::can::Can;
use embassy_stm32::usart::BufferedUart;
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
};
use embedded_io_async::{Read, Write};
use panic_halt as _;
use static_cell::StaticCell;
use swd::Swd;

const PACKET: usize = 64;
// Track linker-script changes so Cargo relinks the firmware.
const _: &[u8] = include_bytes!("../memory.x");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let (driver, ports, swd) = wiring::configure(board::init());
    // Classroom-only test VID/PID, not allocated product identifiers.
    let mut config = embassy_usb::Config::new(0xC0DE, 0xCAFC);
    config.manufacturer = Some("HerKules");
    config.product = Some("DM-MC02 CMSIS-DAP Demo");
    config.max_packet_size_0 = PACKET as u8;
    static DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS: StaticCell<[u8; 128]> = StaticCell::new();
    static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
    static CTL_STATE: StaticCell<State> = StaticCell::new();
    static UART_STATE: StaticCell<State> = StaticCell::new();
    let mut builder = Builder::new(
        driver,
        config,
        DESC.init([0; 256]),
        BOS.init([0; 128]),
        MSOS.init([0; 256]),
        CONTROL.init([0; 64]),
    );
    builder.msos_descriptor(0x0603_0000, 0x20);
    let dap = dap_usb::DapUsb::new(&mut builder);
    let control = CdcAcmClass::new(&mut builder, CTL_STATE.init(State::new()), PACKET as u16);
    let serial = CdcAcmClass::new(&mut builder, UART_STATE.init(State::new()), PACKET as u16);
    let mut device = builder.build();
    join4(
        device.run(),
        control_port(control, ports.can, swd),
        uart_port(serial, ports.uart),
        dap.run(),
    )
    .await;
}

async fn control_port(
    mut usb: CdcAcmClass<'static, Usb>,
    mut can: Can<'static>,
    mut swd: Option<Swd>,
) {
    let mut packet = [0; PACKET];
    loop {
        usb.wait_connection().await;
        let mut line = protocol::Line::new();
        'connection: while let Ok(n) = usb.read_packet(&mut packet).await {
            for &byte in &packet[..n] {
                if let Some(command) = line.push(byte) {
                    let reply = protocol::execute(command, &mut can, &mut swd).await;
                    // Responses are bounded below 64 B, so USB needs no extra ZLP.
                    if usb.write_packet(reply.as_bytes()).await.is_err() {
                        break 'connection;
                    }
                }
            }
        }
    }
}

async fn uart_port(usb: CdcAcmClass<'static, Usb>, uart: Option<BufferedUart<'static>>) {
    let Some(uart) = uart else {
        return;
    };
    let (mut serial_out, mut serial_in) = uart.split();
    let (mut usb_out, mut usb_in) = usb.split();
    loop {
        usb_in.wait_connection().await;
        let from_host = async {
            let mut buf = [0; PACKET];
            while let Ok(n) = usb_in.read_packet(&mut buf).await {
                if serial_out.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
        };
        let to_host = async {
            // A short packet needs no zero-length terminator.
            let mut buf = [0; PACKET - 1];
            while let Ok(n) = serial_in.read(&mut buf).await {
                if n != 0 && usb_out.write_packet(&buf[..n]).await.is_err() {
                    break;
                }
            }
        };
        select(from_host, to_host).await;
    }
}
