
add_library(crt
        src/crt/start.cpp
        src/crt/relocations.cpp)

target_include_directories(crt PUBLIC include)

target_compile_options(crt PRIVATE
        -fno-rtti
        -fno-exceptions
        -std=gnu++20
        -O0
        -g3)

target_compile_options(crt PUBLIC
        -nostdlib
        -nodefaultlibs
        -fpic)

target_link_options(crt PUBLIC
        "-Wl,-pie"
        "-nostdlib"
        "-Wl,-no-dynamic-linker"
        "SHELL:-Wl,-T ${CMAKE_CURRENT_SOURCE_DIR}/p1c0.ld")

