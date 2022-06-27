#[repr(C)]
pub struct RelaEntry {
    offset: usize,
    ty: usize,
    addend: usize,
}

const R_AARCH64_RELATIVE: usize = 1027;

/// Applies relative offsets during boot to relocate the binary.
///
/// # Safety
///   `rela_start` must point to valid memory, at the start of the relocatable information
///   `rela_len_bytes` must be larger than 0 and indicate the size of the slice in bytes.
///   Other regular conditions must hold when calling thsi function (e.g.: having a valid SP)
#[no_mangle]
pub unsafe extern "C" fn apply_rela(
    base: usize,
    rela_start: *const RelaEntry,
    rela_len_bytes: usize,
) {
    let rela_len = rela_len_bytes / core::mem::size_of::<RelaEntry>();
    let relocations = &*core::ptr::slice_from_raw_parts(rela_start, rela_len);

    for relocation in relocations {
        let ptr = (base + relocation.offset) as *mut usize;
        match relocation.ty {
            R_AARCH64_RELATIVE => *ptr = base + relocation.addend,
            _ => unimplemented!(),
        };
    }
}
