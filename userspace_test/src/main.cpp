#include "types.h"
#include "relocations.h"

size_t strlen(const char *str) {
  if (str == nullptr) {
    return 0;
  }

  size_t size = 0;
  while (*str != '\0') {
    size++;
    str++;
  }

  return size;
}

void puts(const char *str) {
  const size_t string_length = strlen(str);
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

extern "C" u8 _rela_start;
extern "C" u8 _rela_end;

bool format_hex(u32 number, char *str, u32 max_length) {
  const u32 mask = 0xF0000000;
  constexpr static auto NIBBLES_IN_U32 = 8;

  if (max_length < NIBBLES_IN_U32 + 1) {
    return false;
  }

  for (u32 i = 0; i < NIBBLES_IN_U32; i++) {
    const auto value = (number & mask) >> 28;
    if (value >= 10) {
      str[i] = 'A' + value - 10;
    } else {
      str[i] = '0' + value;
    }
    number <<= 4;
  }

  str[NIBBLES_IN_U32 + 1] = '\0';
  return true;
}

// base_addr is passed to us via the OS so that we know where the binary was loaded. This can be used for ASLR.
CLINKAGE int _start(u64 base_addr) {
  // After booting we need to apply self-relocations (since this is a pie executable there is no dynamic loader to do
  // any relocations)
  const RelaEntry *relocations = (const RelaEntry *) &_rela_start;
  u64 rela_len_bytes = (u64) & _rela_end - (u64) & _rela_start;
  apply_relocations(base_addr, relocations, rela_len_bytes);

  char str[16];

  int i = 0;
  // And now we can start doing work
  while (true) {
    puts("Hi there!");
    format_hex(i, str, 16);
    puts(str);
    i++;
    sleep(1'000'000);
  }

  return 0;
}
