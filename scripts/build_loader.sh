# Compile kernel
RUSTFLAGS="-C target-cpu=cortex-a72 -C link-arg=--library-path=./kernelloader -C link-arg=--script=kernel_loader.ld" cargo rustc --release --target aarch64-unknown-none-softfloat --bin kernelloader --features kernelloader

# Strip it
rust-objcopy --strip-all -O binary ./target/aarch64-unknown-none-softfloat/release/kernelloader kernelloader8.img
