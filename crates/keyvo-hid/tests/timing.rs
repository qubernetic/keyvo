//! The device-layer defaults equal the `[timing]` defaults of `config.toml`.

use keyvo_core::config::Timing;
use keyvo_hid::hidpp::DEFAULT_REPLY_TIMEOUT;
use keyvo_hid::image::DEFAULT_ACK_TIMEOUT;
use keyvo_hid::keep_alive::{DEFAULT_PERIOD, DEFAULT_TIMEOUT};
use keyvo_hid::reprog::{DEFAULT_DIVERT_SETTLE, DEFAULT_DIVERT_SPACING};

#[test]
fn hid_defaults_equal_the_config_timing_defaults() {
    let timing = Timing::default();
    assert_eq!(DEFAULT_PERIOD, timing.keep_alive_period);
    assert_eq!(DEFAULT_TIMEOUT, timing.keep_alive_timeout);
    assert_eq!(DEFAULT_ACK_TIMEOUT, timing.ack_timeout);
    assert_eq!(DEFAULT_DIVERT_SETTLE, timing.divert_settle);
    assert_eq!(DEFAULT_DIVERT_SPACING, timing.divert_spacing);
    assert_eq!(DEFAULT_REPLY_TIMEOUT, timing.reply_timeout);
}
