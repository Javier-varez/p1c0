
#include <libcxx/types.h>
#include <libcxx/syscalls.h>

namespace libcxx::syscalls {
    static usize strlen(const char *str) {
      if (str == nullptr) {
        return 0;
      }

      usize size = 0;
      while (*str != '\0') {
        size++;
        str++;
      }

      return size;
    }

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

    extern "C" u8 _rela_start;
    extern "C" u8 _rela_end;

    constexpr static u32 NIBBLES_IN_U32 = 16;
    constexpr static u32 HEX_64_BIT_STR_LEN = NIBBLES_IN_U32 + 1;

    static bool format_hex(u64 number, char (&str)[HEX_64_BIT_STR_LEN]) {
      const u64 mask = 0xF000000000000000;

      for (u32 i = 0; i < NIBBLES_IN_U32; i++) {
        const auto value = (number & mask) >> 60;
        if (value >= 10) {
          str[i] = 'A' + value - 10;
        } else {
          str[i] = '0' + value;
        }
        number <<= 4;
      }

      str[NIBBLES_IN_U32] = '\0';
      return true;
    }

    void puthex(u64 value) {
      char str[HEX_64_BIT_STR_LEN];
      format_hex(value, str);
      puts(str);
    }
}