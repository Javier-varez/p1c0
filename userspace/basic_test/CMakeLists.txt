
add_executable(basic_test
        src/main.cpp
        src/relocations.cpp
        src/syscalls.cpp)

target_compile_options(basic_test PRIVATE
        -nostdlib
        -nodefaultlibs
        -fno-rtti
        -fno-exceptions
        -fpic
        -O0
        -g3
        -std=gnu++17)

target_include_directories(basic_test PRIVATE include)

target_link_options(basic_test PRIVATE
        "-Wl,-pie"
        "-nostdlib"
        "-Wl,-no-dynamic-linker"
        "SHELL:-Wl,-T ${CMAKE_CURRENT_SOURCE_DIR}/p1c0.ld")

install(TARGETS basic_test)
