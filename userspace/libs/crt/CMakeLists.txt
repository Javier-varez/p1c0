
add_library(crt
        src/crt/start.cpp
        src/crt/crt.cpp
        src/crt/relocations.cpp)

target_include_directories(crt PUBLIC include)

target_compile_options(crt PRIVATE
        -fno-rtti
        -fno-exceptions
        -std=gnu++20
        -O0
        -g3)

# TODO: Add support for out-of-line atomics to increase performance with LSE. For now they are disabled and old-style
# exclusive load/store/clear instructions are used.
target_compile_options(crt PUBLIC
        -nostdlib
        -nodefaultlibs
        -mno-outline-atomics
        -fpic)

target_link_options(crt PUBLIC
        "-Wl,-pie"
        "-nostdlib"
        "-Wl,-no-dynamic-linker"
        "SHELL:-Wl,-T ${CMAKE_CURRENT_SOURCE_DIR}/ldscript.ld")

