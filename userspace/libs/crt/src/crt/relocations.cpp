#include <crt/relocations.h>

namespace crt::relocations {

    u64 apply_relocations(u64 base, const RelaEntry *relocations, u64 rela_len_bytes) {
      u64 num_relocations = rela_len_bytes / sizeof(RelaEntry);

      for (u64 i = 0; i < num_relocations; i++) {
        if (relocations[i].type == R_AARCH64_RELATIVE) {
          u64 *const ptr = reinterpret_cast<u64 *>(base + relocations[i].offset);
          *ptr = base + relocations[i].addend;
        }
      }
      return 0;
    }

}  // namespace crt::relocations