add_library(libcxx
        src/libcxx/syscalls.cpp
        src/libcxx/fmt.cpp)

target_include_directories(libcxx PUBLIC include)

target_compile_options(libcxx PRIVATE
        -O0
        -g3)

target_compile_options(libcxx PUBLIC
        -ffreestanding
        -fno-rtti
        -fno-exceptions
        -std=gnu++20)

target_link_libraries(libcxx PUBLIC crt)
