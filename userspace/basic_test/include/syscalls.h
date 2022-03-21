#ifndef SYSCALLS_H
#define SYSCALLS_H

#include "types.h"

void puts(const char *str);

void sleep(const u64 time_us);

void puthex(u64 value);

#endif  // SYSCALLS_H
