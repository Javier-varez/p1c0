#ifndef LIBCXX_FMT_H_
#define LIBCXX_FMT_H_

#include <libcxx/string.h>
#include <libcxx/span.h>

namespace libcxx {
    usize sprint(Span<char> buffer, const char *fmt, ...) noexcept __attribute__((format(printf, 2, 3)));

    usize sprintln(Span<char> buffer, const char *fmt, ...) noexcept __attribute__((format(printf, 2, 3)));

    void print(const char *fmt, ...) noexcept __attribute__((format(printf, 1, 2)));

    void println(const char *fmt, ...) noexcept __attribute__((format(printf, 1, 2)));
}

#endif  // LIBCXX_FMT_H_
