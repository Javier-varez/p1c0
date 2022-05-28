
#include <libcxx/types.h>
#include <libcxx/syscalls.h>
#include <libcxx/string.h>

namespace libcxx::syscalls {
    void puts(const char *str) {
      const usize string_length = strlen(str);
      asm volatile(
      "mov x0, %0\n"
      "mov x1, %1\n"
      "svc 6" : : "r" (str), "r" (string_length) : "x0", "x1" );
    }

    void sleep(const u64 time_us) {
      asm volatile(
      "mov x0, %0\n"
      "svc 2" : : "r" (time_us) : "x0");
    }
}