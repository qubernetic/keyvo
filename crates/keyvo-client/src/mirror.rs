//! What the pad shows, as the daemon reports it.

use std::collections::VecDeque;

use keyvo_core::profile::KeySlot;
use keyvo_core::socket::{Context, DeviceStatus, Edge, Event, Reply};
use keyvo_core::state::View;
use keyvo_core::trace::TraceRecord;

/// The client's copy of the daemon's state, filled by [`Mirror::answer`]
/// with the replies to `status`, `subscribe` and `press`, then kept current
/// by [`Mirror::apply`] with the subscription's events.
#[derive(Debug, Clone, Default)]
pub struct Mirror {
    view: Option<View>,
    device: Option<DeviceStatus>,
    context: Option<Context>,
    holding: Option<Context>,
    held: Vec<KeySlot>,
    trace: VecDeque<TraceRecord>,
    trace_capacity: usize,
    config_error: Option<String>,
}

impl Mirror {
    /// An empty mirror that keeps the newest `trace_capacity` trace
    /// entries.
    #[must_use]
    pub fn new(trace_capacity: usize) -> Self {
        Self {
            trace_capacity,
            ..Self::default()
        }
    }

    /// Takes in the reply to a request.
    pub fn answer(&mut self, reply: &Reply) {
        match reply {
            Reply::Status(status) => {
                self.view = Some(status.view.clone());
                self.device = Some(status.device.clone());
                self.set_context(status.context.as_ref());
            }
            Reply::Subscribed(subscribed) => self.view = Some(subscribed.view.clone()),
            Reply::Pressed(pressed) => self.push_trace(pressed.entry.clone()),
            Reply::Done {} => {}
        }
    }

    /// Takes in one event; whether what the pad shows may have changed. A
    /// trace entry and a configuration reload always count: the entry is
    /// new and the profiles were read again.
    pub fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::Key { key, edge } => {
                let at = self.held.binary_search(key);
                match (edge, at) {
                    (Edge::Press, Err(at)) => self.held.insert(at, *key),
                    (Edge::Release, Ok(at)) => {
                        self.held.remove(at);
                    }
                    _ => return false,
                }
                true
            }
            Event::Profile(view) | Event::Page(view) | Event::Mode(view) => {
                replace(&mut self.view, view)
            }
            Event::Device(device) => replace(&mut self.device, device),
            Event::Context(context) => {
                let changed = self.context.as_ref() != Some(context);
                self.set_context(Some(context));
                changed
            }
            Event::Trace { entry } => {
                self.push_trace(entry.clone());
                true
            }
            Event::Config(config) => {
                self.config_error = if config.reloaded {
                    None
                } else {
                    config.error.clone()
                };
                true
            }
        }
    }

    /// What the pad shows, once known.
    #[must_use]
    pub const fn view(&self) -> Option<&View> {
        self.view.as_ref()
    }

    /// The Keypad's connection, once known.
    #[must_use]
    pub const fn device(&self) -> Option<&DeviceStatus> {
        self.device.as_ref()
    }

    /// The focused window as last reported.
    #[must_use]
    pub const fn context(&self) -> Option<&Context> {
        self.context.as_ref()
    }

    /// While the focused window holds the context (`held`), the window
    /// reported before it, whose context stays; `None` when this mirror
    /// saw no such report.
    #[must_use]
    pub const fn holding(&self) -> Option<&Context> {
        self.holding.as_ref()
    }

    /// A held context keeps the window before it as `holding`; any other
    /// ends the hold.
    fn set_context(&mut self, context: Option<&Context>) {
        if context.is_some_and(|context| context.held) {
            if let Some(before) = self.context.take().filter(|before| !before.held) {
                self.holding = Some(before);
            }
        } else {
            self.holding = None;
        }
        self.context = context.cloned();
    }

    /// The keys held down on the device, in key order.
    #[must_use]
    pub fn held(&self) -> &[KeySlot] {
        &self.held
    }

    /// The newest trace entries, oldest first.
    #[must_use]
    pub const fn trace(&self) -> &VecDeque<TraceRecord> {
        &self.trace
    }

    /// Why the last reload failed, until one succeeds.
    #[must_use]
    pub fn config_error(&self) -> Option<&str> {
        self.config_error.as_deref()
    }

    fn push_trace(&mut self, entry: TraceRecord) {
        self.trace.push_back(entry);
        while self.trace.len() > self.trace_capacity {
            self.trace.pop_front();
        }
    }
}

/// Sets `slot` to `value`; whether it changed.
fn replace<T: Clone + PartialEq>(slot: &mut Option<T>, value: &T) -> bool {
    if slot.as_ref() == Some(value) {
        return false;
    }
    *slot = Some(value.clone());
    true
}
