add_executable(basic_test src/main.cpp)
target_link_libraries(basic_test PRIVATE libcxx)
install(TARGETS basic_test)
