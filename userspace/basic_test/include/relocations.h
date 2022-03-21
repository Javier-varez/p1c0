#ifndef RELOCATIONS_H_
#define RELOCATIONS_H_

#include "types.h"

struct RelaEntry {
    u64 offset;
    u64 type;
    u64 addend;
};

constexpr static auto R_AARCH64_RELATIVE = 1027;

u64 apply_relocations(u64 base, const RelaEntry* rela_entry, u64 rela_len_bytes);

#endif  // RELOCATIONS_H_
