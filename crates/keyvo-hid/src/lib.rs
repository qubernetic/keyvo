//! HID transport, HID++ 2.0 core and the VLP display plane for the Logitech
//! MX Creative Keypad and Dialpad. `unsafe` is allowed here for the hidapi
//! FFI that lands later; `unsafe_op_in_unsafe_fn` is denied workspace-wide.

#![deny(clippy::unwrap_used, clippy::expect_used)]

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}
