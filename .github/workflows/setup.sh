#!/bin/bash -xe

TOOLS_DIR=.tools

QEMU_VERSION=0.1.3
QEMU_DIR=${PWD}/${TOOLS_DIR}/qemu

OS=$(uname | tr '[:upper:]' '[:lower:]')
QEMU_ZIP=${QEMU_VERSION}_M1_Pro_${OS}.zip
QEMU_URL=https://github.com/Javier-varez/qemu-apple-m1/releases/download/Apple_M1_Pro_${QEMU_VERSION}/${QEMU_ZIP}

ZIP_DIR=${PWD}
wget ${QEMU_URL}
mkdir -p ${QEMU_DIR}

pushd ${QEMU_DIR}
unzip ${ZIP_DIR}/${QEMU_ZIP}
chmod +x ./bin/qemu-system-aarch64
popd

rm -r ${QEMU_ZIP}

# Make sure qemu binaries end up in the PATH environment variable
echo "${QEMU_DIR}/bin" >> ${GITHUB_PATH}

# Install other dependencies
sudo apt update
sudo apt install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    binutils-aarch64-linux-gnu
cargo install cargo-binutils
