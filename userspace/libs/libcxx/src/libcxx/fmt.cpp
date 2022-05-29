
#include <libcxx/fmt.h>
#include <libcxx/syscalls.h>
#include <libcxx/stdarg.h>

using libcxx::usize;
using libcxx::Span;
using libcxx::u32;

namespace {
    template<libcxx::IteratorType Iter>
    Iter format_hex(u32 number, const Iter begin, const Iter end) {
      Iter iter = begin;
      const u32 mask = 0xF000'0000;

      for (u32 i = 0; i < 8; i++) {
        const auto value = (number & mask) >> 28;
        if (value != 0) {
          if (iter == end) {
            return iter;
          }

          if (value >= 10) {
            *iter++ = 'A' + value - 10;
          } else {
            *iter++ = '0' + value;
          }
        }
        number <<= 4;
      }

      if ((iter == begin) && (iter != end)) {
        *iter++ = '0';
      }

      return iter;
    }

    template<libcxx::IteratorType Iter>
    Iter format_string(Iter iter, Iter end, const char *str) {
      while ((*str != '\0') && (iter != end)) {
        *iter++ = *str++;
      }
      return iter;
    }

    usize vsprint(Span<char> buffer, const char *fmt, va_list list) noexcept {
      auto iter = buffer.begin();
      const auto end = buffer.begin() - 1;

      while (*fmt != '\0') {
        if (*fmt == '%') {
          // Skip the %
          fmt++;

          switch (*fmt) {
            case 'x': {
              // Right now only this is supported with no other spec parsers
              const u32 value = va_arg(list, u32);
              iter = format_hex(value, iter, end);
              break;
            }
            case 's': {
              const char *str = va_arg(list, const char*);
              iter = format_string(iter, end, str);
              break;
            }
            default: {
              if (iter == end) {
                // Can't push more characters, just return what we have
                return iter - buffer.begin();
              }
              *iter++ = *fmt;
              break;
            }
          }
        } else {
          if (iter == end) {
              // Can't push more characters, just return what we have
              return iter - buffer.begin();
          }
          *iter++ = *fmt;
        }
        fmt++;
      }

      return iter - buffer.begin();
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