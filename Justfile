build_release:
  cargo build --release

build_debug:
  cargo build

run_debug:
  RUST_LOG=trace cargo run

run_release: build_release
  RUST_LOG=info ./target/release/swaddle

clean:
  cargo clean
