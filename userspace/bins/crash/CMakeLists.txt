add_executable(crash src/main.cpp)
target_link_libraries(crash PRIVATE libcxx)
install(TARGETS crash)
