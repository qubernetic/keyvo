//! A subscription followed to its end: what the desktop app's Home mirror
//! listens to.

use std::io::{Read, Write};

use keyvo_core::socket::Event;
use keyvo_core::state::View;
use serde::Serialize;

use crate::{Client, ClientError};

/// What a subscription hands on, in order: the view, every event, then why
/// it ended. Serialized tagged by `kind`, with the event as on the wire
/// without `v`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Update {
    /// The subscription stands; the pad shows `view`.
    Subscribed {
        /// What the pad shows.
        view: View,
    },
    /// One event.
    Event {
        /// The event.
        event: Event,
    },
    /// The subscription ended: the daemon could not be reached, refused,
    /// hung up or sent a line this client does not read.
    Closed {
        /// Why, for the user.
        reason: String,
    },
}

/// Subscribes on `client` to every event class after the newest `replay`
/// trace entries and hands each [`Update`] to `sink` until the
/// subscription ends, the last update being [`Update::Closed`]. Stops
/// early, without `Closed`, once `sink` returns `false`. `client` is the
/// result of connecting, so a daemon that cannot be reached ends the same
/// way.
pub fn follow<S: Read + Write>(
    client: Result<Client<S>, ClientError>,
    replay: usize,
    mut sink: impl FnMut(Update) -> bool,
) {
    let reason = match stream(client, replay, &mut sink) {
        Ok(true) => ClientError::Closed,
        Ok(false) => return,
        Err(error) => error,
    };
    sink(Update::Closed {
        reason: reason.to_string(),
    });
}

/// Streams until the daemon hangs up (`true`) or `sink` stops (`false`).
fn stream<S: Read + Write>(
    client: Result<Client<S>, ClientError>,
    replay: usize,
    sink: &mut impl FnMut(Update) -> bool,
) -> Result<bool, ClientError> {
    let mut client = client?;
    let view = client.subscribe(replay)?;
    if !sink(Update::Subscribed { view }) {
        return Ok(false);
    }
    while let Some(event) = client.next_event()? {
        if !sink(Update::Event { event }) {
            return Ok(false);
        }
    }
    Ok(true)
}
