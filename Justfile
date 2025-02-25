default: run_debug

# runs the build release process
build_release:
  cargo build --release

# builds a debug version of swaddle
build_debug:
  cargo build

# runs swaddle w debug via `cargo run`
run_debug:
  RUST_LOG=debug cargo run

# run the release version of swaddle
run_release: build_release
  ./target/release/swaddle

# cleans up build artifacts
clean:
  cargo clean
