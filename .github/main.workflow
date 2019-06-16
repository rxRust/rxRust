workflow "Build and Publish on push" {
  on = "push"
  resolves = ["build-and-test"]
}

action "build-and-test" {
  uses = "docker://rustlang/rust:nightly"
  runs = ["sh", "-c", "rustup component add rustfmt && cargo fmt -- --check && rustup component add clippy && cargo clippy -- -Dwarnings && cargo clippy --all-targets --all-features -- -D warnings && cargo test"]
}
