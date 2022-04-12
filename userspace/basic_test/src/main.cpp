#include "types.h"
#include "relocations.h"
#include "syscalls.h"

[[gnu::noinline]] void oh_my_bug(u64 i);

[[gnu::noinline]] void print_message(u64 i);

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
CLINKAGE [[noreturn]] void _start(u64 base_addr) {
  // After booting we need to apply self-relocations (since this is a pie executable there is no dynamic loader to do
  // any relocations)
  const RelaEntry *relocations;
  u64 rela_len_bytes;

  asm volatile("adr %0, _rela_start\n"
               "adr %1, _rela_end\n"
               "sub %1, %1, %0\n" :
  "=r" (relocations), "=r" (rela_len_bytes)::);

  apply_relocations(base_addr, relocations, rela_len_bytes);

  u64 i = (u64) & _start;
  // And now we can start doing work
  while (true) {
    print_message(i);
    i++;
    sleep(1'000'000);
  }
}

[[gnu::noinline]] void oh_my_bug(u64 i) {
  if (i == 0x3000005) {
    // Crash the hell out of this process
    volatile int *ptr = nullptr;
    *ptr = 123;
  }
}

[[gnu::noinline]] void print_message(u64 i) {
  puts("Hi there!");
  puthex(i);
  oh_my_bug(i);
}
