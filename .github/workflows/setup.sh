#!/bin/bash -xe

TOOLS_DIR=.tools
QEMU_DIR=${PWD}/${TOOLS_DIR}/qemu
GCC_DIR=${PWD}/${TOOLS_DIR}/gcc-aarch64-none-elf
DOWNLOAD_DIR=${PWD}/${TOOLS_DIR}/downloads

QEMU_VERSION="0.1.4"
GCC_VERSION="11.2-2022.02"

OS=$(uname | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m| tr '[:upper:]' '[:lower:]')
if [ "$ARCH" == "arm64" ]; then
ARCH=aarch64
fi

download_tool() {
    mkdir -p ${DOWNLOAD_DIR}
    wget -P ${DOWNLOAD_DIR} $1
}

get_qemu_install_path() {
    echo "${QEMU_DIR}/bin"
}

get_gcc_install_path() {
    if [ "darwin" == $OS ]; then
        OS_ADDEND="-darwin"
    else
        OS_ADDEND=""
    fi

    echo "${GCC_DIR}/gcc-arm-${GCC_VERSION}${OS_ADDEND}-${ARCH}-aarch64-none-elf/bin"
}

download_qemu() {
    ZIP=${QEMU_VERSION}_M1_Pro_${OS}_${ARCH}.zip
    URL=https://github.com/Javier-varez/qemu-apple-m1/releases/download/Apple_M1_Pro_${QEMU_VERSION}/${ZIP}

    download_tool ${URL}

    mkdir -p ${QEMU_DIR}
    pushd ${QEMU_DIR}
    unzip ${DOWNLOAD_DIR}/${ZIP}
    chmod +x ./bin/qemu-system-aarch64
    popd

    # Cleanup
    rm -rf ${DOWNLOAD_DIR}

}

download_gcc() {
    if [ "darwin" == $OS ]; then
        OS_ADDEND="-darwin"
        GCC_ARCH=x86_64
    else
        OS_ADDEND=""
        GCC_ARCH=${ARCH}
    fi

    INSTALL_DIR=${TOOLS_DIR}/gcc-aarch64-none-elf/

    TAR_NAME=gcc-arm-${GCC_VERSION}${OS_ADDEND}-${GCC_ARCH}-aarch64-none-elf.tar.xz
    URL=https://developer.arm.com/-/media/Files/downloads/gnu/11.2-2022.02/binrel/${TAR_NAME}
    download_tool ${URL}

    mkdir -p ${GCC_DIR}
    pushd ${GCC_DIR}
    tar -xf ${DOWNLOAD_DIR}/${TAR_NAME}
    popd

    # Cleanup
    rm -rf ${DOWNLOAD_DIR}

}

ensure_qemu() {
    QEMU_INSTALL_PATH=$(get_qemu_install_path)
    if [ -d "$(get_qemu_install_path)" ]; then
        echo "qemu already installed"
    else
        download_qemu
    fi

    if [ -n "${GITHUB_PATH}" ]; then
        echo "${QEMU_INSTALL_PATH}" >> ${GITHUB_PATH}
    fi

    export PATH="${QEMU_INSTALL_PATH}:${PATH}"
}

ensure_gcc() {
    GCC_INSTALL_PATH=$(get_gcc_install_path)
    if [ -d "${GCC_INSTALL_PATH}" ]; then
        echo "gcc already installed"
    else
        download_gcc
    fi

    if [ -n "${GITHUB_PATH}" ]; then
        echo "${GCC_INSTALL_PATH}" >> ${GITHUB_PATH}
    fi

    export PATH="${GCC_INSTALL_PATH}:${PATH}"
}

write_env() {
    GCC_INSTALL_PATH=$(get_gcc_install_path)
    QEMU_INSTALL_PATH=$(get_qemu_install_path)

    echo "PATH += ${GCC_INSTALL_PATH}:${QEMU_INSTALL_PATH}" > .env
}

ensure_qemu
ensure_gcc

write_env

cargo install cargo-binutils
cargo install grcov
