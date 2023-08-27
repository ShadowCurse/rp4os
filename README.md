# rp4os

Os for rasberry pie 4.
Based on [rust-raspberrypi-OS-tutorials](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)

To build kernel run:
```bash
./scripts/build.sh
```

To build kernel loader run:
```bash
./scripts/build_loader.sh
```

Use `kernelloader` as an initial kernel and use `bool_console`
to connect to it and with uart and upload new kernel to run.

To build `bool_console` run:
```bash
cd bool_console
cargo build --release --target x86_64-unknown-linux-gnu
```

To launch `bool_console` run:
```bash
sudo ./target/x86_64-unknown-linux-gnu/release/boot_console --device /dev/ttyUSB0 --baud 921600 --kernel ../kernel8.img
```

Pressing `1` will start kernel transfer.
After the transfer is finished new kernel will start it's execution.
