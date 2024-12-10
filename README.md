mithril
=======

Firmware for the Sting flight computer.

![](system_architecture.png)

# Setup

## STLink drivers

**Linux**: Install via package manager if possible, e.g. `pacman -S stlink`

**Windows**:
- Download drivers from [ST's website](https://www.st.com/en/development-tools/stsw-link009.html) (requires account)
- Extract .zip and run `dpinst_amd64.exe` for 64-bit systems, and `dpinst_x86.exe` on 32-bit systems.

After installation, it *may* be necessary to unplug the STLink and plug it back in, or even reboot (or reload udev rules on Linux).

## Rustup

A tool used to manage different Rust versions and targets (e.g. x86, ARM, etc.). Download and run installer: follow the instructions on https://rustup.rs/ (Use rustup-init.exe for Windows, defaults are fine)

## Rust

Using `rustup`, we can download the target needed for the STM32 and some other tools we need.

Run the following commands (you can skip ones needed for flashing methods you're not interested in):

  - `rustup target add thumbv7em-none-eabihf` (Add ARM toolchain)
  - `cargo install probe-rs-tools --locked`
  - `cargo install flip-link (Needed for cargo run)`

## Sam Ground Station

Once Rust is installed, install [Sam](https://github.com/tudsat-rocket/sam).

# Compiling & Flashing

## Serial Wire Debug (SWD)

This method requires an STLink v2 programmer, connected to the FC via the SWD header. To compile and flash the firmware:

```
cargo make swd
```

For this method is is irrelevant if the STM32 is in bootloader mode.

## Device Firmware Update via USB

This method only requires a USB connection.

```
cargo run --release
```

Flashing via DFU requires the STM32 to be in bootloader mode. The above command will attempt to establish a serial connection to the STM32 and request a reboot to bootloader, but this may fail. To manually reboot the FC into the bootloader, hold the "BOOT" button down while pressing the "RESET" button once.
