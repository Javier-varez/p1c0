
#include <libcxx/fmt.h>
#include <libcxx/syscalls.h>
#include <libcxx/stdarg.h>

using libcxx::usize;
using libcxx::Span;
using libcxx::u32;

namespace {
    usize format_hex(u32 number, Span<char> buffer) {
      usize offset = 0;
      const u32 mask = 0xF000'0000;

      for (u32 i = 0; i < 8; i++) {
        const auto value = (number & mask) >> 28;
        if (value != 0) {
          if (offset >= buffer.size() - 1) {
            return offset;
          }

          if (value >= 10) {
            buffer[offset++] = 'A' + value - 10;
          } else {
            buffer[offset++] = '0' + value;
          }
        }
        number <<= 4;
      }

      if (offset == 0) {
        // No characters were written, write a 0 at least
        if (offset >= buffer.size() - 1) {
          return offset;
        }
        buffer[offset++] = '0';
      }

      return offset;
    }

    usize format_string(Span<char> buffer, const char *str) {
      usize offset = 0;
      while ((*str != '\0') && (offset < (buffer.size() - 1))) {
        buffer[offset++] = *str++;
      }
      return offset;
    }

    usize vsprint(Span<char> buffer, const char *fmt, va_list list) noexcept {
      usize offset = 0;
      while (*fmt != '\0') {
        if (*fmt == '%') {
          // Skip the %
          fmt++;

          switch (*fmt) {
            case 'x': {
              // Right now only this is supported with no other spec parsers
              const u32 value = va_arg(list, u32);
              offset += format_hex(value, buffer.from_offset(offset));
              break;
            }
            case 's': {
              const char *str = va_arg(list, const char*);
              offset += format_string(buffer.from_offset(offset), str);
              break;
            }
            default: {
              if (offset >= buffer.size() - 1) {
                // Can't push more characters, just return what we have
                return offset;
              }
              buffer[offset++] = *fmt;
              break;
            }
          }
        } else {
          if (offset >= buffer.size() - 1) {
            // Can't push more characters, just return what we have
            return offset;
          }
          buffer[offset++] = *fmt;
        }
        fmt++;
      }

      return offset;
    }
}

namespace libcxx {
    usize sprint(Span<char> buffer, const char *fmt, ...) noexcept {
      va_list args;
      va_start(args, fmt);
      usize offset = vsprint(buffer, fmt, args);
      va_end(args);
      buffer[offset++] = '\0';
      return offset;
    }

    usize sprintln(Span<char> buffer, const char *fmt, ...) noexcept {
      va_list args;
      va_start(args, fmt);
      usize offset = vsprint(buffer, fmt, args);
      va_end(args);

      if (offset < buffer.size() - 1) {
        buffer[offset++] = '\n';
      }
      buffer[offset++] = '\0';
      return offset;
    }

    void print(const char *fmt, ...) noexcept {
      // TODO(javier-varez): Use buffered IO instead of this piece of crap array thing
      Array<char, 512> string;

      va_list args;
      va_start(args, fmt);
      usize offset = vsprint(string, fmt, args);
      va_end(args);

      string[offset++] = '\0';
      libcxx::syscalls::puts(&string[0]);
    }

    void println(const char *fmt, ...) noexcept {
      // TODO(javier-varez): Use buffered IO instead of this piece of crap array thing
      Array<char, 512> string;

      va_list args;
      va_start(args, fmt);
      usize offset = vsprint(string, fmt, args);
      va_end(args);

      if (offset < string.size() - 1) {
        string[offset++] = '\n';
      }
      string[offset++] = '\0';
      libcxx::syscalls::puts(&string[0]);
    }
}