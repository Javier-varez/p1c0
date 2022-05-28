#include <libcxx/types.h>
#include <libcxx/syscalls.h>
#include <libcxx/fmt.h>

using libcxx::u64;
using libcxx::usize;

namespace {
    [[gnu::noinline]] void oh_my_bug(u64 i, bool withTrick);

    [[gnu::noinline]] void print_message(u64 i, bool withTrick);
}

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
int main(int argc, char *argv[], char *envp[]) {
  int i = 0;

  libcxx::print("Num arguments is %x", argc);
  for (usize i = 0; i < argc; i++) {
    libcxx::print("Argument %x is `%s`", i, argv[i]);
  }

  while (i < 5) {
    print_message(i, argc > 1);
    i++;
    libcxx::syscalls::sleep(1'000'000);
  }

  return 0;
}

namespace {
    class Guard {
    public:
        Guard() {
          libcxx::print("C++ global constructors work!");
        }

        ~Guard() {
          libcxx::print("C++ global destructors also work!");
        }
    };

    __attribute__((constructor)) void constructor() {
      libcxx::print("C constructor functions work!");
    }

    __attribute__((destructor)) void destructor() {
      libcxx::print("C destructor functions work!");
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
      libcxx::print("Hi there! %x", i);
      oh_my_bug(i, withTrick);
    }
}