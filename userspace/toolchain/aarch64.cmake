set(CMAKE_SYSTEM_NAME, P1C0)
set(CMAKE_SYSTEM_PROCESSOR, arm64)
set(CMAKE_SYSTEM_VERSION, 1.0)

set(triple aarch64-unknown-none-softfloat)
set(CMAKE_C_COMPILER aarch64-none-elf-gcc)
set(CMAKE_C_COMPILER_TARGET ${triple})
set(CMAKE_)

set(CMAKE_CXX_COMPILER aarch64-none-elf-g++)
set(CMAKE_CXX_COMPILER_TARGET ${triple})

# We need to make sure to not omit frame pointer to make debugging easier.
# Otherwise we might not be able to get backtraces
set(CMAKE_CXX_FLAGS "-fno-omit-frame-pointer")
set(CMAKE_C_FLAGS "-fno-omit-frame-pointer")

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
set(CMAKE_C_COMPILER_WORKS TRUE)
set(CMAKE_CXX_COMPILER_WORKS TRUE)
