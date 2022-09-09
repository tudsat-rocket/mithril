- Install Rustup
- Install DFU-Util (`pacman -S dfu-util`)
- `rustup target install thumbv7em-none-eabihf`
- `rustup toolchain install nightly --target thumbv7em-none-eabihf`
- `rustup component add --toolchain nightly llvm-tools-preview`
- `cargo install cargo-binutils`
- `cargo install cargo-make`
- `cargo make dfu` or
- `cargo make dfu-monitor` or