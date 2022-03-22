# p1c0

Although p1c0 started as a playground and research tool for M1 macs, it is currently heading towards
a simple kernel and maybe complete OS in the future targeting the 2021 MacBook Pro 14".

The OS is mostly written in Rust, with some assembly bits here and there to interact with the
hardware. At the moment it has support for basic threading in EL1 and some drivers to interact with
the hardware (like an HID driver for the keyboard through the SPI transport protocol).

## Getting started

Get the sources from [GitHub](https://github.com/javier-varez/p1c0) with:

```bash
git clone https://github.com/javier-varez/p1c0
```

Assuming you have cargo installed in your system you will need a couple more dependencies to build
and test the project:

```bash
# Install cargo-binutils, used to generate a binary/Mach-o file out of the compiled ELF.
cargo install cargo-binutils

# Assuming you are building this in Ubuntu-20.04. Otherwise check your package manager
# On an m1 mac with macOS p1c0 will just use the built-in clang version
sudo apt update 
sudo apt install -y gcc-aarch64-linux-gnu
```

In order to run the simulator you will need a version of Qemu with support for the Apple M1 Pro. You
can find this version [here](https://github.com/javier-varez/qemu-apple-m1), and the latest
release [here](https://github.com/Javier-varez/qemu-apple-m1/releases/tag/Apple_M1_Pro_0.1.3).

```bash
INSTALL_DIR=${YOUR_DESIRED_INSTALL_PATH}
OS=$(uname | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
curl -OL https://github.com/Javier-varez/qemu-apple-m1/releases/download/Apple_M1_Pro_0.1.3/0.1.3_M1_Pro_${OS}_${ARCH}.zip
unzip -d ${INSTALL_DIR}/qemu-apple-m1 0.1.3_M1_Pro_${OS}_${ARCH}.zip

# And finally make it available in your path. You can add this to your .bashrc or .bash_profile
export PATH=${INSTALL_DIR}/qemu-apple-m1/bin:$PATH
```

### Running the emulator

```bash
cargo rr
```

### Building the code targeting the real MacBook Pro 14"

```bash
cargo br
```

This will create a `.macho` file in `fw/p1c0.macho`. To install this object into your computer, you
can follow the instructions
[here](https://github.com/AsahiLinux/docs/wiki/Developer-Quickstart#setup).

### Running tests

```bash
$ cargo t
```

## Contributing

Feel free to contribute to this project and open issues. Appreciated contributions include, but are
not limited to:

* Bug reports
* Code contributions
* Documentation contributions
* Issues
* Feature requests

Regarding code contributions, make sure to format all code with `rust-fmt`.

## Acknowledgements

Some of this code is based on the fantastic research done by [marcan](https://github.com/marcan)
and the good people behind the [Asahi Linux](https://github.com/AsahiLinux) project.

In addition, some early code (like the exceptions.rs file) was based on the
[rust-raspberrypi-OS-tutorial](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)
from the [rust-embedded organization](https://github.com/rust-embedded).
