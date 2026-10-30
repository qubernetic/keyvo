//! Test doubles for the daemon: [`Pad`], a Keypad built on
//! `FakeHidTransport`, and [`Pads`], a connector that hands out pads; [`Dial`],
//! a Bolt receiver with the Dialpad, and [`Dials`], its connector.
//!
//! The scripted part (keep-alive lookup, claim, divert) is replayed byte
//! for byte by `FakeHidTransport`. Image fragments are the one thing a
//! script cannot hold, because their bytes are the renderer's output: the
//! pad takes them like the device does, acknowledges each fragment on
//! `0x13` in the captured layout (`13 ff <display> <fn|swid> c<index> 00
//! <index>`) and records every finished image with its rectangle. It also
//! echoes every keep-alive ping, whenever the actor sends one, so the tests
//! run with the default `[timing]` period (spec §6.1 refuses a period that
//! is not below the timeout).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use keyvo_core::hid::{FakeHidTransport, HidTransport, TransportError};
use keyvo_daemon::device::Connector;
use keyvo_daemon::dialpad::DialpadConnector;
use keyvo_hid::dialpad::Link;

/// The display's VLP index in the fixtures.
pub const DISPLAY: u8 = 2;
/// The `0x1B04` feature index in the fixtures.
pub const REPROG: u8 = 0x0b;
/// The head of a keepAlive(3000) ping in the fixtures (index 0x04, fn1,
/// swid 0xe); the pad echoes it like the device, whenever it comes.
const KEEP_ALIVE_PING: [u8; 6] = [0x11, 0xff, 0x04, 0x1e, 0x0b, 0xb8];
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

/// The device reports (`<` lines) of a fixture script, in order.
pub fn reports(script: &str) -> Vec<Vec<u8>> {
    script
        .lines()
        .filter_map(|line| line.strip_prefix('<'))
        .map(|hex| {
            hex.split_whitespace()
                .map(|byte| u8::from_str_radix(byte, 16).unwrap())
                .collect()
        })
        .collect()
}

/// What a connection to the Keypad starts with: the `KEEP_ALIVE` lookup,
/// the display claim, the display capabilities (fn0, for `maxImageSize`;
/// the display index is known from the claim) and the page-button divert.
/// The pings themselves are answered by the pad.
pub fn startup() -> String {
    format!(
        "{}{}{}{}",
        exchanges(&fixture("keep-alive-ping.txt"), 0..1),
        fixture("display-claim.txt"),
        exchanges(&fixture("display-capabilities.txt"), 1..2),
        fixture("reprog-divert.txt"),
    )
}

/// [`startup`] of a Keypad just plugged in: its claim sees the plug-in
/// splash (`display-splash.txt`, constructed from the #52 daemon log; no
/// raw capture of a replug exists) twice before `a0` takes.
pub fn startup_in_splash() -> String {
    let claim = fixture("display-claim.txt");
    let head = claim.split("# 6.").next().unwrap();
    startup().replace(&claim, &format!("{head}{}", fixture("display-splash.txt")))
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
    sent: VecDeque<Vec<u8>>,
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
            sent: VecDeque::new(),
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

    /// The page buttons `cids` are held now: a diverted-control
    /// notification on the `0x1B04` index, as in `page-buttons.txt`.
    pub fn buttons(&self, cids: &[u16]) {
        let mut report = vec![0x11, 0xff, REPROG, 0x00];
        for cid in cids {
            report.extend_from_slice(&cid.to_be_bytes());
        }
        report.resize(20, 0);
        self.state().input.push_back(report);
    }

    /// The device sends `reports` next, together and ahead of the scripted
    /// replies, as the two events of a keep-alive expiry come within one
    /// millisecond, before any answer to what keyvo writes after them.
    pub fn send(&self, reports: &[Vec<u8>]) {
        self.state().sent.extend(reports.iter().cloned());
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
        if report.starts_with(&KEEP_ALIVE_PING) {
            state.acks.push_back(report.to_vec());
            return Ok(());
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
        // The device's ACK layout (image-acks.txt): 01 on the last fragment.
        let mut ack = vec![
            0x13,
            report[1],
            report[2],
            report[3],
            0xc0 | index,
            0x00,
            u8::from(seq & 0x40 != 0),
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
            match state.sent.pop_front().or_else(|| state.acks.pop_front()) {
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

/// The Dialpad's slot on the receiver in the fixtures.
pub const DIAL_SLOT: u8 = 3;
/// The Dialpad's `0x1B04` index in the fixtures.
pub const DIAL_REPROG: u8 = 0x0a;
/// The Dialpad's `0x4610` index in the fixtures.
pub const DIAL_ROLLERS: u8 = 0x0d;
/// The CIDs of the Dialpad's buttons 1 to 4 in the fixtures.
pub const DIAL_BUTTONS: [u16; 4] = [0x53, 0x56, 0x59, 0x5a];

/// What a connection to the Dialpad on the receiver starts with: the slot
/// search, the `0x1B04` walk, the rollers and every divert.
pub fn dial_startup() -> String {
    [
        fixture("receiver-slot-probe.txt"),
        fixture("receiver-controls.txt"),
        fixture("receiver-multiroller.txt"),
        fixture("receiver-divert.txt"),
    ]
    .concat()
}

#[derive(Debug)]
struct DialState {
    script: FakeHidTransport,
    input: VecDeque<Vec<u8>>,
    unplugged: bool,
}

/// A Bolt receiver with the Dialpad on slot 3 for the daemon tests: the
/// script (startup, restore) replayed byte for byte, the turns and presses
/// a test makes sent as the Dialpad sends them. Clones share the device.
#[derive(Debug, Clone)]
pub struct Dial(Arc<Mutex<DialState>>);

impl Dial {
    /// A receiver that replays `script`.
    pub fn new(script: &str) -> Self {
        Self(Arc::new(Mutex::new(DialState {
            script: FakeHidTransport::from_script(script).unwrap(),
            input: VecDeque::new(),
            unplugged: false,
        })))
    }

    fn state(&self) -> MutexGuard<'_, DialState> {
        self.0.lock().unwrap()
    }

    /// Roller `roller` (0 the roller, 1 the dial) turns by `delta`.
    pub fn turn(&self, roller: u8, delta: i8) {
        let mut report = vec![
            0x11,
            DIAL_SLOT,
            DIAL_ROLLERS,
            0x00,
            roller,
            delta.to_be_bytes()[0],
        ];
        report.resize(20, 0);
        self.state().input.push_back(report);
    }

    /// Exactly the buttons `buttons` (1 to 4) are held now.
    pub fn hold(&self, buttons: &[u8]) {
        let mut report = vec![0x11, DIAL_SLOT, DIAL_REPROG, 0x00];
        for button in buttons {
            report.extend_from_slice(&DIAL_BUTTONS[usize::from(*button - 1)].to_be_bytes());
        }
        report.resize(20, 0);
        self.state().input.push_back(report);
    }

    /// The receiver goes away: every later read and write fails.
    pub fn unplug(&self) {
        self.state().unplugged = true;
    }

    /// True when every scripted request was written and every scripted
    /// report was read.
    pub fn script_done(&self) -> bool {
        self.state().script.is_done()
    }
}

impl HidTransport for Dial {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        let mut state = self.state();
        if state.unplugged {
            return Err(TransportError::Disconnected);
        }
        state.script.write(report)
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let report = {
            let mut state = self.state();
            if state.unplugged {
                return Err(TransportError::Disconnected);
            }
            match state.script.read(timeout)? {
                Some(report) => Some(report),
                None => state.input.pop_front(),
            }
        };
        if report.is_none() {
            std::thread::sleep(timeout.min(IDLE_READ));
        }
        Ok(report)
    }
}

/// Hands out the given receivers in order, a `None` failing like a
/// missing receiver, then fails for good.
#[derive(Debug)]
pub struct Dials(pub VecDeque<Option<Dial>>);

impl Dials {
    /// A connector for `dials`.
    pub fn new(dials: impl IntoIterator<Item = Dial>) -> Self {
        Self(dials.into_iter().map(Some).collect())
    }

    /// A connector that finds no receiver `misses` times, then `dial`.
    pub fn after(misses: usize, dial: Dial) -> Self {
        let mut dials: VecDeque<_> = std::iter::repeat_n(None, misses).collect();
        dials.push_back(Some(dial));
        Self(dials)
    }
}

impl DialpadConnector for Dials {
    type Transport = Dial;

    fn connect(&mut self) -> Result<(Dial, Link), TransportError> {
        self.0
            .pop_front()
            .flatten()
            .map(|dial| (dial, Link::Receiver))
            .ok_or_else(|| TransportError::Io("no receiver".into()))
    }
}
