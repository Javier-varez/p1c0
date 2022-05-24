#ifndef CRT_RELOCATIONS_H_
#define CRT_RELOCATIONS_H_

namespace crt::relocations {
    using u64 = __UINT64_TYPE__;

    struct RelaEntry {
        u64 offset;
        u64 type;
        u64 addend;
    };

    constexpr static auto R_AARCH64_RELATIVE = 1027;

    u64 apply_relocations(u64 base, const RelaEntry *rela_entry, u64 rela_len_bytes);

}  // namespace crt::relocations

#endif  // CRT_RELOCATIONS_H_
