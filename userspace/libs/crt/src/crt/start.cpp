#include <crt/relocations.h>

namespace {
    [[noreturn]] void exit(crt::relocations::u64 exit_code) {
      asm volatile(
      "mov x0, %0\n"
      "svc 8" : : "r" (exit_code) : "x0");
      __builtin_unreachable();
    }
}

int main();

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
extern "C" [[noreturn]] void _start(crt::relocations::u64 base_addr) {
  // After booting we need to apply self-relocations (since this is a pie executable there is no dynamic loader to do
  // any relocations)
  const crt::relocations::RelaEntry *relocations;
  crt::relocations::u64 rela_len_bytes;

  asm volatile("adr %0, _rela_start\n"
               "adr %1, _rela_end\n"
               "sub %1, %1, %0\n" :
  "=r" (relocations), "=r" (rela_len_bytes)::);

  crt::relocations::apply_relocations(base_addr, relocations, rela_len_bytes);

  const auto retval = main();
  exit(retval);

  while (true);
}