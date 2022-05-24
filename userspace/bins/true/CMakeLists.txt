add_executable(true src/main.cpp)
target_link_libraries(true PRIVATE libcxx)
install(TARGETS true)
