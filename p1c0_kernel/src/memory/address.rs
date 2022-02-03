use crate::arch::mmu::PAGE_SIZE;

/// This is the base address for logical addresses.
const KERNEL_LOGICAL_BASE: LogicalAddress =
    unsafe { LogicalAddress::new_unchecked(0xFFFF020000000000 as *const u8) };
const KERNEL_LOGICAL_SIZE: usize = 128 * 1024 * 1024 * 1024 * 1024; // 128 TB

pub trait Address {
    fn as_ptr(&self) -> *const u8;

    fn as_usize(&self) -> usize {
        self.as_ptr() as usize
    }

    fn as_u64(&self) -> u64 {
        self.as_ptr() as u64
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    UnalignedAddress,
    AddressOutOfRange,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct VirtualAddress(*const u8);

unsafe impl Send for VirtualAddress {}

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

    /// # Safety
    ///   The user must guarantee that the resulting pointer is a valid VirtualAddress after this
    ///   operation. This means that it is within the limits of addressable virtual memory.
    #[must_use]
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
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

unsafe impl Send for PhysicalAddress {}

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