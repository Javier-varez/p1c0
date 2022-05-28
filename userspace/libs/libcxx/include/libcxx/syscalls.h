#ifndef LIBCXX_SYSCALLS_H_
#define LIBCXX_SYSCALLS_H_

#include <libcxx/types.h>

namespace libcxx::syscalls {
    /**
     * @brief Writes the given string to stdout
     */
    void puts(const char *str);

    /**
     * @brief Sleeps for the given number of nanoseconds
     */
    void sleep(u64 time_us);
}

#endif  // LIBCXX_SYSCALLS_H_
