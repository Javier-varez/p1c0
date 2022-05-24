add_executable(false src/main.cpp)
target_link_libraries(false PRIVATE libcxx)
install(TARGETS false)
