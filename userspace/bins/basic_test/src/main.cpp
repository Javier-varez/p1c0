#include <libcxx/types.h>
#include <libcxx/syscalls.h>

using libcxx::u64;

namespace {
    [[gnu::noinline]] void oh_my_bug(u64 i);

    [[gnu::noinline]] void print_message(u64 i);
}

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
int main() {
  int i = 0;
  // And now we can start doing work
  while (true) {
    print_message(i);
    i++;
    libcxx::syscalls::sleep(1'000'000);
  }
}

namespace {
    [[gnu::noinline]] void oh_my_bug(u64 i) {
      if (i == 5) {
        // Crash the hell out of this process
        volatile int *ptr = nullptr;
        *ptr = 123;
      }
    }

    [[gnu::noinline]] void print_message(u64 i) {
      libcxx::syscalls::puts("Hi there!");
      libcxx::syscalls::puthex(i);
      oh_my_bug(i);
    }
}