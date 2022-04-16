use crate::{
    arch::mmu::{PAGE_BITS, PAGE_SIZE},
    memory::map::{KERNEL_LOGICAL_BASE, KERNEL_LOGICAL_SIZE},
};

pub trait Validator {
    /// Validates virtual addresses are within a valid address range.
    fn is_valid(&self, va: VirtualAddress) -> bool;
}

pub trait Address {
    #[must_use]
    fn as_ptr(&self) -> *const u8;

    #[must_use]
    fn as_usize(&self) -> usize {
        self.as_ptr() as usize
    }

    #[must_use]
    fn as_u64(&self) -> u64 {
        self.as_ptr() as u64
    }

    #[must_use]
    fn is_page_aligned(&self) -> bool {
        (self.as_usize() & (PAGE_SIZE - 1)) == 0
    }

    #[must_use]
    fn page_number(&self) -> u64 {
        self.as_u64() >> PAGE_BITS
    }

    #[must_use]
    fn as_mut_ptr(&self) -> *mut u8 {
        self.as_ptr() as *mut _
    }

    #[must_use]
    fn is_null(&self) -> bool {
        self.as_ptr().is_null()
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    UnalignedAddress,
    AddressOutOfRange,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct VirtualAddress(*const u8);

impl VirtualAddress {
    /// # Safety
    ///   The pointer must be a valid virtual address
    pub const unsafe fn new_unchecked(ptr: *const u8) -> Self {
        Self(ptr)
    }

    pub fn try_from_ptr(addr: *const u8) -> Result<Self, Error> {
        let addr_usize = addr as usize;
        if (addr_usize & (PAGE_SIZE - 1)) != 0 {
            return Err(Error::UnalignedAddress);
        }
        Ok(Self(addr))
    }

    pub fn new_unaligned(ptr: *const u8) -> Self {
        Self(ptr)
    }

    /// # Safety
    ///   The user must guarantee that the resulting pointer is a valid VirtualAddress after this
    ///   operation. This means that it is within the limits of addressable virtual memory.
    #[must_use]
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
    }

    #[must_use]
    pub fn remove_base(&self, other: VirtualAddress) -> Self {
        let val = unsafe { self.0.offset_from(other.0) };
        assert!(val > 0);
        Self(val as *const _)
    }

    pub fn is_high_address(&self) -> bool {
        let high_bits = self.0 as usize >> 48;
        if high_bits == 0xFFFF {
            true
        } else if high_bits == 0x0000 {
            false
        } else {
            panic!("Virtual address is invalid");
        }
    }
}

impl Address for VirtualAddress {
    fn as_ptr(&self) -> *const u8 {
        self.0
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct PhysicalAddress(*const u8);

impl PhysicalAddress {
    #[must_use]
    /// # Safety
    ///   the pointer must be a valid physical address
    pub const unsafe fn new_unchecked(ptr: *const u8) -> Self {
        Self(ptr)
    }

    pub fn try_from_ptr(addr: *const u8) -> Result<Self, Error> {
        let addr_usize = addr as usize;
        if (addr_usize & (PAGE_SIZE - 1)) != 0 {
            return Err(Error::UnalignedAddress);
        }
        Ok(Self(addr))
    }

    #[must_use]
    pub fn from_unaligned_ptr(addr: *const u8) -> Self {
        Self(addr)
    }

    #[must_use]
    pub fn align_to_page(&self) -> Self {
        let mut addr_usize = self.0 as usize;
        addr_usize &= !(PAGE_SIZE - 1);
        Self(addr_usize as *const u8)
    }

    /// # Safety
    ///   The user must guarantee that the resulting pointer is a valid PhysicalAddress after this
    ///   operation. This means that it is within the limits of addressable physical memory and
    ///   points to a valid physical address backed by some memory device (either memory mapped IO or
    ///   regular memory).
    #[must_use]
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
    }

    pub fn try_into_logical(&self) -> Result<LogicalAddress, Error> {
        if cfg!(test) {
            Ok(unsafe { LogicalAddress::new_unchecked(self.as_ptr()) })
        } else {
            LogicalAddress::try_from_ptr(unsafe {
                self.as_ptr().add(KERNEL_LOGICAL_BASE.as_usize())
            })
        }
    }

    pub fn offset_from(&self, other: PhysicalAddress) -> isize {
        let self_isize = self.as_usize() as isize;
        let other_isize = other.as_usize() as isize;
        self_isize.wrapping_sub(other_isize)
    }
}

impl Address for PhysicalAddress {
    fn as_ptr(&self) -> *const u8 {
        self.0
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct LogicalAddress(*const u8);

impl LogicalAddress {
    /// # Safety
    ///   the pointer must be a valid logical address
    #[must_use]
    pub const unsafe fn new_unchecked(ptr: *const u8) -> Self {
        Self(ptr)
    }

    // This lint error seems to be triggering even though there is no pointer dereference here
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn try_from_ptr(ptr: *const u8) -> Result<Self, Error> {
        let offset = unsafe { ptr.offset_from(KERNEL_LOGICAL_BASE.as_ptr()) };
        // We don't check pointers in unittests because we cannot control where they get allocated
        if !cfg!(test) && (offset < 0 || offset > KERNEL_LOGICAL_SIZE as isize) {
            return Err(Error::AddressOutOfRange);
        }
        Ok(Self(ptr))
    }

    #[must_use]
    pub fn into_physical(&self) -> PhysicalAddress {
        if cfg!(test) {
            unsafe { PhysicalAddress::new_unchecked(self.as_ptr()) }
        } else {
            // # Safety
            // A logical address always has a corresponding physical address
            unsafe {
                PhysicalAddress::new_unchecked(
                    self.as_ptr()
                        .offset(-(KERNEL_LOGICAL_BASE.as_usize() as isize)),
                )
            }
        }
    }

    #[must_use]
    pub fn into_virtual(&self) -> VirtualAddress {
        unsafe { VirtualAddress::new_unchecked(self.as_ptr()) }
    }
}

impl Address for LogicalAddress {
    fn as_ptr(&self) -> *const u8 {
        self.0
    }
}

impl core::fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtualAddress({:?})", self.as_ptr())
    }
}

impl core::fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalAddress({:?})", self.as_ptr())
    }
}

impl core::fmt::Display for LogicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "LogicalAddress({:?})", self.as_ptr())
    }
}

// All memory addresses can be shared freely between threads,
unsafe impl Send for VirtualAddress {}

unsafe impl Send for PhysicalAddress {}

unsafe impl Send for LogicalAddress {}
