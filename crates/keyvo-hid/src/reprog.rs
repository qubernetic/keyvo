//! Classic HID++ `0x1B04 REPROG_CONTROLS_V4` (spec §5.6): the control walk
//! and the divert of the Keypad's page buttons, whose presses then arrive
//! as notifications ([`crate::input::Input::with_page_buttons`]).

/// CID of the left page button (measured).
pub const PAGE_LEFT: u16 = 0x01A1;
/// CID of the right page button (measured).
pub const PAGE_RIGHT: u16 = 0x01A2;
