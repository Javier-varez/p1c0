#ifndef LIBCXX_STRING_H_
#define LIBCXX_STRING_H_

#include <libcxx/types.h>

namespace libcxx {
    constexpr usize strlen(const char *str) noexcept {
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
}

#endif  // LIBCXX_STRING_H_
