# Compile kernel
RUSTFLAGS="-C target-cpu=cortex-a72 -C link-arg=--library-path=./src/bsp -C link-arg=--script=kernel.ld" cargo rustc --release --target aarch64-unknown-none-softfloat

# Strip it
rust-objcopy --strip-all -O binary ./target/aarch64-unknown-none-softfloat/release/kernel kernel8.img
