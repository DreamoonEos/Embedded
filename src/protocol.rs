use core::fmt::Write;
use embassy_stm32::can::{BufferedCanSender, Frame};
use embedded_can::{Frame as _, Id};
use heapless::String;

use crate::swd::Swd;

pub(crate) type Reply = String<63>;

pub struct Line {
    bytes: [u8; 96],
    len: usize,
}

impl Line {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 96],
            len: 0,
        }
    }

    pub fn push(&mut self, byte: u8) -> Option<&str> {
        if byte == b'\n' {
            let n = core::mem::take(&mut self.len);
            // A full buffer may contain a truncated command. Discard that line.
            if n == self.bytes.len() {
                return Some("");
            }
            return Some(core::str::from_utf8(&self.bytes[..n]).unwrap_or_default());
        }
        if self.len < self.bytes.len() {
            self.bytes[self.len] = byte;
            self.len += 1;
        }
        None
    }
}

fn fixed(text: &str) -> Reply {
    Reply::try_from(text).expect("fixed reply fits one short USB packet")
}

pub fn is_can_rx(command: &str) -> bool {
    let mut words = command.split_ascii_whitespace();
    words.next() == Some("CAN") && words.next() == Some("RX") && words.next().is_none()
}

/// `polled` selects the reply to `CAN RX`. Other frames use the unsolicited form.
pub fn format_frame(frame: &Frame, polled: bool) -> Option<Reply> {
    let Id::Standard(id) = *frame.id() else {
        return None;
    };
    if frame.is_remote_frame() {
        return None;
    }
    let mut out = Reply::new();
    let kind = if polled { "OK RX" } else { "RX" };
    write!(out, "{kind} {:03X} ", id.as_raw()).ok()?;
    if frame.data().is_empty() {
        out.push('-').ok()?;
    }
    for byte in frame.data() {
        write!(out, "{byte:02X}").ok()?;
    }
    out.push('\n').ok()?;
    Some(out)
}

pub async fn execute(command: &str, can: &mut BufferedCanSender, swd: &mut Option<Swd>) -> Reply {
    let mut words = command.split_ascii_whitespace();
    let mut out = Reply::new();
    match (words.next(), words.next()) {
        (Some("PING"), None) => return fixed("OK DM-MC02\n"),
        (Some("SWD"), Some("ID")) if words.next().is_none() => {
            let Some(swd) = swd.as_mut() else {
                return fixed("ERR SWD_NOT_CONFIGURED\n");
            };
            match swd.read_dpidr() {
                Some(id) => writeln!(out, "OK DPIDR {id:08X}").expect("SWD reply fits"),
                None => return fixed("ERR SWD\n"),
            }
        }
        (Some("CAN"), Some("TX")) => {
            let (Some(id), Some(hex), None) = (words.next(), words.next(), words.next()) else {
                return fixed("ERR SYNTAX\n");
            };
            let Some(frame) = parse_frame(id, hex) else {
                return fixed("ERR FRAME\n");
            };
            // Full software queue: refuse immediately so this task does not wait on the bus.
            return match can.try_write(frame) {
                Ok(()) => fixed("OK QUEUED\n"),
                Err(_) => fixed("ERR CAN TX\n"),
            };
        }
        _ => return fixed("ERR SYNTAX\n"),
    }
    out
}

fn parse_frame(id: &str, payload: &str) -> Option<Frame> {
    let id = u16::from_str_radix(id, 16).ok()?;
    let mut bytes = [0; 8];
    let payload = if payload == "-" { "" } else { payload };
    if !payload.is_ascii() || payload.len() > 16 || !payload.len().is_multiple_of(2) {
        return None;
    }
    for (i, byte) in bytes[..payload.len() / 2].iter_mut().enumerate() {
        *byte = u8::from_str_radix(&payload[i * 2..i * 2 + 2], 16).ok()?;
    }
    Frame::new_standard(id, &bytes[..payload.len() / 2]).ok()
}
