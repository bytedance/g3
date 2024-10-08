check:
    cargo check --workspace

test:
    cargo test --workspace --lib --examples

clippy:
    cargo clippy --tests -- --deny warnings

qa: check test clippy
