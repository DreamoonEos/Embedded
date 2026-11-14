//! CAN bridge on the USB control port.
//!
//! Host to board, one line ending in `\n`: `CAN TX 321 A55A`
//! Board to host, as soon as a classical standard frame arrives: `RX 321 A55A`
//! `CAN RX` still waits up to 100 ms and answers `OK RX 321 A55A` or `EMPTY`.
//! Empty payload is `-`. ID is 3 hex digits. Data is hex with no spaces.
//!
//! `protocol::Line` is the frame boundary. Bytes are kept until `\n`, so one
//! request may span USB packets. The byte loop runs every finished line, so
//! one packet may contain several requests.
//!
//! Three tasks share the work. `host_in` reads the host. `can_in` reads CAN
//! frames that the FDCAN interrupt already pushed into an 8-deep queue.
//! `host_out` is the only task that writes USB. A full CAN queue drops the new
//! frame inside the interrupt. A full host queue drops a forwarded frame.
//! A command reply waits at most 50 ms, then is dropped. `CAN TX` uses
//! `try_write`: a full TX queue answers `ERR CAN TX` and does not wait.
//! If the host stops reading, only `host_out` waits. If USB write fails, that
//! task drops queued lines and waits for the next connection.

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_futures::join::join3;
use embassy_stm32::can::{BufferedCanReceiver, BufferedCanSender, Can, RxBuf, TxBuf};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, with_timeout};
use embassy_usb::class::cdc_acm::{Receiver, Sender};
use static_cell::StaticCell;

use crate::board::Usb;
use crate::protocol::{self, Line, Reply};
use crate::swd::Swd;

const QUEUE: usize = 8;
const PACKET: usize = 64;

static OUT: Channel<CriticalSectionRawMutex, Reply, QUEUE> = Channel::new();
static TXQ: StaticCell<TxBuf<QUEUE>> = StaticCell::new();
static RXQ: StaticCell<RxBuf<QUEUE>> = StaticCell::new();
static POLL: AtomicBool = AtomicBool::new(false);
static POLL_DONE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub async fn run(
    usb: embassy_usb::class::cdc_acm::CdcAcmClass<'static, Usb>,
    can: Can<'static>,
    swd: Option<Swd>,
) {
    let can = can.buffered(TXQ.init(Channel::new()), RXQ.init(Channel::new()));
    let (usb_tx, usb_rx) = usb.split();
    join3(
        host_in(usb_rx, can.writer(), swd),
        can_in(can.reader()),
        host_out(usb_tx),
    )
    .await;
}

async fn host_in(
    mut usb: Receiver<'static, Usb>,
    mut can: BufferedCanSender,
    mut swd: Option<Swd>,
) {
    let mut packet = [0; PACKET];
    loop {
        usb.wait_connection().await;
        let mut line = Line::new();
        while let Ok(n) = usb.read_packet(&mut packet).await {
            for &byte in &packet[..n] {
                if let Some(command) = line.push(byte) {
                    dispatch(command, &mut can, &mut swd).await;
                }
            }
        }
    }
}

async fn dispatch(command: &str, can: &mut BufferedCanSender, swd: &mut Option<Swd>) {
    if protocol::is_can_rx(command) {
        if !poll_frame().await {
            emit(must("EMPTY\n")).await;
        }
        return;
    }
    emit(protocol::execute(command, can, swd).await).await;
}

async fn poll_frame() -> bool {
    POLL_DONE.reset();
    POLL.store(true, Ordering::Release);
    let heard = with_timeout(Duration::from_millis(100), POLL_DONE.wait())
        .await
        .is_ok()
        || POLL_DONE.try_take().is_some();
    POLL.store(false, Ordering::Release);
    heard
}

async fn can_in(rx: BufferedCanReceiver) {
    loop {
        let Ok(envelope) = rx.receive().await else {
            continue;
        };
        let polled = POLL.swap(false, Ordering::AcqRel);
        let line = match protocol::format_frame(&envelope.frame, polled) {
            Some(line) => line,
            None if polled => must("ERR UNSUPPORTED_FRAME\n"),
            None => continue,
        };
        let queued = OUT.try_send(line).is_ok();
        if polled && queued {
            POLL_DONE.signal(());
        }
    }
}

async fn host_out(mut usb: Sender<'static, Usb>) {
    loop {
        usb.wait_connection().await;
        loop {
            let line = OUT.receive().await;
            if usb.write_packet(line.as_bytes()).await.is_err() {
                while OUT.try_receive().is_ok() {}
                break;
            }
        }
    }
}

async fn emit(line: Reply) {
    let _ = with_timeout(Duration::from_millis(50), OUT.send(line)).await;
}

fn must(text: &str) -> Reply {
    Reply::try_from(text).expect("fixed reply fits")
}
