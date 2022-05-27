#include <libcxx/types.h>
#include <libcxx/syscalls.h>

using libcxx::u64;
using libcxx::usize;

namespace {
    [[gnu::noinline]] void oh_my_bug(u64 i, bool withTrick);

    [[gnu::noinline]] void print_message(u64 i, bool withTrick);
}

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
int main(int argc, char *argv[], char *envp[]) {
  int i = 0;

  libcxx::syscalls::puts("Num arguments is");
  libcxx::syscalls::puthex(argc);
  for (usize i = 0; i < argc; i++) {
    libcxx::syscalls::puts(argv[i]);
  }

  while (i < 5) {
    print_message(i, argc > 1);
    i++;
    libcxx::syscalls::sleep(1'000'000);
  }
}

namespace {
    class Guard {
    public:
        Guard() {
          libcxx::syscalls::puts("C++ global constructors work!");
        }

        ~Guard() {
          libcxx::syscalls::puts("C++ global destructors also work!");
        }
    };

    __attribute__((constructor)) void constructor() {
      libcxx::syscalls::puts("C constructor functions work!");
    }

    __attribute__((destructor)) void destructor() {
      libcxx::syscalls::puts("C destructor functions work!");
    }

    Guard guard;

    [[gnu::noinline]] void oh_my_bug(u64 i, bool withTrick) {
      if ((i == 3) && withTrick) {
        // Crash the hell out of this process
        volatile int *ptr = nullptr;
        *ptr = 123;
      }
    }

    [[gnu::noinline]] void print_message(u64 i, bool withTrick) {
      libcxx::syscalls::puts("Hi there!");
      libcxx::syscalls::puthex(i);
      oh_my_bug(i, withTrick);
    }
}