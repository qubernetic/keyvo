//! Test doubles for the daemon: [`Pad`], a Keypad built on
//! `FakeHidTransport`, and [`Pads`], a connector that hands out pads.
//!
//! The scripted part (keep-alive, claim, divert) is replayed byte for byte
//! by `FakeHidTransport`. Image fragments are the one thing a script cannot
//! hold, because their bytes are the renderer's output: the pad takes them
//! like the device does, acknowledges each fragment on `0x13` in the
//! captured layout (`13 ff <display> <fn|swid> c<index> 00 <index>`) and
//! records every finished image with its rectangle.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use keyvo_core::hid::{FakeHidTransport, HidTransport, TransportError};
use keyvo_daemon::device::Connector;

/// The display's VLP index in the fixtures.
pub const DISPLAY: u8 = 2;
/// Longest a read of an idle pad blocks, so the actor does not spin.
const IDLE_READ: Duration = Duration::from_millis(2);

/// A fixture script from `tests/protocol-fixtures/hid/`.
pub fn fixture(name: &str) -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/protocol-fixtures/hid/"
    );
    std::fs::read_to_string(format!("{path}{name}")).unwrap()
}

/// The exchanges `range` of a fixture script (a `>` line with the `<`
/// lines after it, counted from 0), comments dropped.
pub fn exchanges(script: &str, range: std::ops::Range<usize>) -> String {
    let mut exchange = None::<usize>;
    let mut out = String::new();
    for line in script.lines() {
        if line.starts_with('>') {
            exchange = Some(exchange.map_or(0, |n| n + 1));
        }
        let wanted = exchange.is_some_and(|n| range.contains(&n));
        if wanted && line.starts_with(['>', '<']) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// What a connection to the Keypad starts with: the first keep-alive ping,
/// the display claim and the page-button divert.
pub fn startup() -> String {
    format!(
        "{}{}{}",
        exchanges(&fixture("keep-alive-ping.txt"), 0..2),
        fixture("display-claim.txt"),
        fixture("reprog-divert.txt"),
    )
}

/// One image the pad received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// x, y, width, height.
    pub rect: (u16, u16, u16, u16),
    /// The JPEG, without the padding.
    pub jpeg: Vec<u8>,
}

#[derive(Debug)]
struct Partial {
    rect: (u16, u16, u16, u16),
    len: usize,
    jpeg: Vec<u8>,
}

#[derive(Debug)]
struct State {
    script: FakeHidTransport,
    acks: VecDeque<Vec<u8>>,
    input: VecDeque<Vec<u8>>,
    partial: Option<Partial>,
    images: Vec<Image>,
    unplugged: bool,
    seq: u8,
}

/// A Keypad for the daemon tests. Clones share the device, so a test keeps
/// one and gives the other to the daemon.
#[derive(Debug, Clone)]
pub struct Pad(Arc<Mutex<State>>);

impl Pad {
    /// A pad that replays `script` for everything but image writes.
    pub fn new(script: &str) -> Self {
        Self(Arc::new(Mutex::new(State {
            script: FakeHidTransport::from_script(script).unwrap(),
            acks: VecDeque::new(),
            input: VecDeque::new(),
            partial: None,
            images: Vec::new(),
            unplugged: false,
            seq: 0xc0,
        })))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    /// Reports that exactly `keys` are held (an empty slice releases all).
    pub fn hold(&self, keys: &[u8]) {
        let mut state = self.state();
        let mut report = vec![0x13, 0xff, DISPLAY, 0x00, state.seq, 0x01];
        report.extend_from_slice(keys);
        report.resize(32, 0);
        state.seq = state.seq.wrapping_add(1);
        state.input.push_back(report);
    }

    /// Every image received so far, in order.
    pub fn images(&self) -> Vec<Image> {
        self.state().images.clone()
    }

    /// The device goes away: every later read and write fails.
    pub fn unplug(&self) {
        self.state().unplugged = true;
    }

    /// True when every scripted request was written and every scripted
    /// report was read.
    pub fn script_done(&self) -> bool {
        self.state().script.is_done()
    }
}

impl HidTransport for Pad {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        let mut state = self.state();
        if state.unplugged {
            return Err(TransportError::Disconnected);
        }
        if report.first() != Some(&0x14) {
            return state.script.write(report);
        }
        let seq = report[4];
        let index = seq & 0x0f;
        if seq & 0x80 != 0 {
            let at = |i: usize| u16::from_be_bytes([report[i], report[i + 1]]);
            let len = usize::from(report[17]) << 16
                | usize::from(report[18]) << 8
                | usize::from(report[19]);
            state.partial = Some(Partial {
                rect: (at(9), at(11), at(13), at(15)),
                len,
                jpeg: report[20..].to_vec(),
            });
        } else if let Some(partial) = &mut state.partial {
            partial.jpeg.extend_from_slice(&report[5..]);
        }
        if seq & 0x40 != 0
            && let Some(mut partial) = state.partial.take()
        {
            partial.jpeg.truncate(partial.len);
            state.images.push(Image {
                rect: partial.rect,
                jpeg: partial.jpeg,
            });
        }
        let mut ack = vec![
            0x13,
            report[1],
            report[2],
            report[3],
            0xc0 | index,
            0x00,
            index,
        ];
        ack.resize(32, 0);
        state.acks.push_back(ack);
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let report = {
            let mut state = self.state();
            if state.unplugged {
                return Err(TransportError::Disconnected);
            }
            match state.acks.pop_front() {
                Some(ack) => Some(ack),
                None => match state.script.read(timeout)? {
                    Some(report) => Some(report),
                    None => state.input.pop_front(),
                },
            }
        };
        if report.is_none() {
            std::thread::sleep(timeout.min(IDLE_READ));
        }
        Ok(report)
    }
}

/// Hands out the given pads in order, then fails like a missing device.
#[derive(Debug)]
pub struct Pads(pub VecDeque<Pad>);

impl Pads {
    /// A connector for `pads`.
    pub fn new(pads: impl IntoIterator<Item = Pad>) -> Self {
        Self(pads.into_iter().collect())
    }
}

impl Connector for Pads {
    type Transport = Pad;

    fn connect(&mut self) -> Result<Pad, TransportError> {
        self.0
            .pop_front()
            .ok_or_else(|| TransportError::Io("no keypad".into()))
    }
}
