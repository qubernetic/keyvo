//! keyvo background service. Prints its version and exits until the socket
//! server, window watchers and input injector land (roadmap M1 to M3).

fn main() {
    println!("keyvo-daemon {}", keyvo_core::VERSION);
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}
