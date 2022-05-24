#ifndef LIBCXX_SYSCALLS_H
#define LIBCXX_SYSCALLS_H

#include <libcxx/types.h>

namespace libcxx::syscalls {
    /**
     * @brief Writes the given string to stdout
     */
    void puts(const char *str);

    /**
     * @brief Sleeps for the given number of nanoseconds
     */
    void sleep(const u64 time_us);

    /**
     * @brief Writes the given value as hex to stdout
     */
    void puthex(u64 value);
}

#endif  // LIBCXX_SYSCALLS_H
