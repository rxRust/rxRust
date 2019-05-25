workflow "Build and Publish on push" {
  on = "push"
  resolves = ["build-and-test"]
}

action "build-and-test" {
  uses = "docker://rust:1.35"
  runs = ["sh", "-c", "rustup toolchain install nightly && rustup component add rustfmt --toolchain nightly && cargo +nightly fmt -- --check && rustup component add clippy && cargo clippy -- -Dwarnings && cargo test"]
}
