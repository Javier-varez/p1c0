cmake_minimum_required(VERSION 3.16)

# We are cross-compiling
set(CMAKE_SYSTEM_NAME Generic)
set(CMAKE_SYSTEM_VERSION 1.0)

project(userspace_test)

add_executable(userspace_test
        src/main.cpp
        src/relocations.cpp)

target_compile_options(userspace_test PRIVATE
        -nostdlib
        -nodefaultlibs
        -fno-rtti
        -fno-exceptions
        -fpic
        -O0
        -g3
        -std=gnu++17)

target_include_directories(userspace_test PRIVATE include)

target_link_options(userspace_test PRIVATE
        "-Wl,-pie"
        "-nostdlib"
        "-Wl,-no-dynamic-linker"
        "SHELL:-Wl,-T ${CMAKE_CURRENT_SOURCE_DIR}/p1c0.ld")
