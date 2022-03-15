#include "types.h"
#include "relocations.h"

u64 apply_relocations(u64 base, const RelaEntry* relocations, u64 rela_len_bytes) {
  u64 num_relocations = rela_len_bytes / sizeof(RelaEntry);
  const RelaEntry* const last_relocation = relocations + num_relocations;

  while (relocations < last_relocation) {
    if (relocations->type == R_AARCH64_RELATIVE) {
      u64 * const ptr = reinterpret_cast<u64*>(base + relocations->offset);
      *ptr = base + relocations->addend;
    }
    relocations++;
  }
  return 0;
}
