extern crate alloc;

use alloc::boxed::Box;

use super::address::{Address, PhysicalAddress};
use crate::{
    arch::mmu::PAGE_BITS,
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    log_info,
};

#[derive(Debug, Clone)]
pub enum Error {
    RegionNotAvailable,
    /// Contains the overlap region
    RegionOverlapsWith(PhysicalAddress, usize),
}

fn pfn_from_pa(pa: PhysicalAddress) -> usize {
    assert!(pa.is_page_aligned());

    pa.as_usize() >> PAGE_BITS
}

pub struct PhysicalPage {
    _pfn: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalMemoryRegion {
    pa: PhysicalAddress,
    num_pages: usize,
}

impl PhysicalMemoryRegion {
    fn new(pa: PhysicalAddress, num_pages: usize) -> Self {
        Self { pa, num_pages }
    }

    fn overlaps(&self, pa: PhysicalAddress, num_pages: usize) -> bool {
        let self_pfn_start = pfn_from_pa(self.pa);
        let self_pfn_end = self_pfn_start + self.num_pages;
        let other_pfn_start = pfn_from_pa(pa);
        let other_pfn_end = other_pfn_start + num_pages;

        self_pfn_start < other_pfn_end && self_pfn_end > other_pfn_start
    }

    fn can_be_consolidated_with(&self, pa: PhysicalAddress, num_pages: usize) -> bool {
        let self_pfn_start = pfn_from_pa(self.pa);
        let self_pfn_end = self_pfn_start + self.num_pages;
        let other_pfn_start = pfn_from_pa(pa);
        let other_pfn_end = other_pfn_start + num_pages;

        self_pfn_start == other_pfn_end || self_pfn_end == other_pfn_start
    }

    fn contains(&self, pa: PhysicalAddress, num_pages: usize) -> bool {
        let self_pfn_start = pfn_from_pa(self.pa);
        let self_pfn_end = self_pfn_start + self.num_pages;
        let other_pfn_start = pfn_from_pa(pa);
        let other_pfn_end = other_pfn_start + num_pages;

        other_pfn_start >= self_pfn_start && other_pfn_end <= self_pfn_end
    }

    fn matches_start(&self, pa: PhysicalAddress) -> bool {
        let self_pfn_start = pfn_from_pa(self.pa);
        let other_pfn_start = pfn_from_pa(pa);

        self_pfn_start == other_pfn_start
    }

    fn matches_end(&self, pa: PhysicalAddress, num_pages: usize) -> bool {
        let self_pfn_start = pfn_from_pa(self.pa);
        let self_pfn_end = self_pfn_start + self.num_pages;
        let other_pfn_start = pfn_from_pa(pa);
        let other_pfn_end = other_pfn_start + num_pages;

        self_pfn_end == other_pfn_end
    }
}

pub struct PhysicalPageAllocator {
    regions: IntrusiveList<PhysicalMemoryRegion>,
}

impl PhysicalPageAllocator {
    pub const fn new() -> Self {
        Self {
            regions: IntrusiveList::new(),
        }
    }

    pub(super) fn add_region(
        &mut self,
        pa: PhysicalAddress,
        num_pages: usize,
    ) -> Result<(), Error> {
        log_info!(
            "PhysicalPageAllocator - Adding region with base address {}, num_pages {}",
            pa,
            num_pages
        );

        if let Some(region) = self
            .regions
            .iter()
            .find(|region| region.overlaps(pa, num_pages))
        {
            return Err(Error::RegionOverlapsWith(region.pa, region.num_pages));
        }

        // No overlap, we can add the region.

        // We pull the regions that could be consolidated with this one
        let mut ranges_to_join = self
            .regions
            .drain_filter(|region| region.can_be_consolidated_with(pa, num_pages));

        if !ranges_to_join.is_empty() {
            // Not only we can consolidate entries, but we also do this without carrying out any
            // allocations, which is truly nice.

            // We can unwrap now because we know the list is not empty
            let min_pa = core::cmp::min(
                pa.as_usize(),
                ranges_to_join
                    .iter()
                    .map(|range| range.pa.as_usize())
                    .reduce(core::cmp::min)
                    .unwrap(),
            );

            let total_num_pages = num_pages
                + ranges_to_join
                    .iter()
                    .map(|range| range.num_pages)
                    .reduce(|num_pages1, num_pages2| num_pages1 + num_pages2)
                    .unwrap();

            // Take one of the ranges and reuse it's object for the new range. This saves
            // allocating another object
            let mut range = ranges_to_join.pop().unwrap();

            // This has to be aligned, and we know it is because we never insert ranges that aren't
            range.pa = PhysicalAddress::try_from_ptr(min_pa as *const _).unwrap();
            range.num_pages = total_num_pages;

            // Re-insert the range into the list of available regions
            self.regions.push(range);

            ranges_to_join.release(|region| {
                // # Safety: We currently allocate with regular box.
                let boxed = unsafe { region.into_box() };
                drop(boxed);
            });
        } else {
            let region = Box::new(IntrusiveItem::new(PhysicalMemoryRegion::new(pa, num_pages)));
            self.regions.push(OwnedMutPtr::new_from_box(region));
        }

        Ok(())
    }

    // Steals regions that are already used for other purposes (other FW, kernel, adt, framebuffer,
    // etc)
    pub(super) fn steal_region(
        &mut self,
        pa: PhysicalAddress,
        num_pages: usize,
    ) -> Result<(), Error> {
        log_info!(
            "PhysicalPageAllocator - Stealing region with base address {}, num_pages {}",
            pa,
            num_pages
        );

        let mut contained_ranges = self
            .regions
            .drain_filter(|region| region.contains(pa, num_pages));

        if contained_ranges.is_empty() {
            return Err(Error::RegionNotAvailable);
        }

        // A region cannot be contained in more than one range. If this happens, there is a bug
        // in our program
        if contained_ranges.iter().count() != 1 {
            panic!("More than one physical region contains a given range, this is a bug!");
        }

        let mut region = contained_ranges.pop().unwrap();

        let matches_start = region.matches_start(pa);
        let matches_end = region.matches_end(pa, num_pages);

        if matches_start && matches_end {
            // Region is completely removed, nothing to do
            let region = unsafe { region.into_box() };
            drop(region);
        } else if matches_start {
            region.pa = unsafe { pa.offset(num_pages << PAGE_BITS) };
            region.num_pages -= num_pages;
            self.regions.push(region);
        } else if matches_end {
            region.num_pages -= num_pages;
            self.regions.push(region);
        } else {
            // We need to split the region unfortunately, which means allocating another region
            // object
            //
            let first_pa = region.pa;
            let first_num_pages = pa.offset_from(first_pa) as usize >> PAGE_BITS;

            let second_pa = unsafe { pa.offset(num_pages << PAGE_BITS) };
            let second_num_pages =
                region.num_pages - (second_pa.offset_from(first_pa) as usize >> PAGE_BITS);

            // We reuse the region object for the first element
            region.pa = first_pa;
            region.num_pages = first_num_pages;
            self.regions.push(region);

            let new_region = Box::new(IntrusiveItem::new(PhysicalMemoryRegion::new(
                second_pa,
                second_num_pages,
            )));
            self.regions.push(OwnedMutPtr::new_from_box(new_region));
        }

        Ok(())
    }

    pub fn print_regions(&self) {
        log_info!("Available physical memory regions:");
        for region in self.regions.iter() {
            let start_addr = region.pa;
            let end_addr = unsafe { region.pa.offset(region.num_pages << PAGE_BITS) };
            log_info!("\t{} -> {}", start_addr, end_addr);
        }
    }

    pub fn request_pages(
        &mut self,
        pa: PhysicalAddress,
        num_pages: usize,
    ) -> Result<PhysicalMemoryRegion, Error> {
        self.steal_region(pa, num_pages)?;
        Ok(PhysicalMemoryRegion::new(pa, num_pages))
    }

    pub fn release_pages(&mut self, region: PhysicalMemoryRegion) -> Result<(), Error> {
        self.add_region(region.pa, region.num_pages)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn add_region() {
        let mut allocator = PhysicalPageAllocator::new();
        let dram_base = PhysicalAddress::try_from_ptr(0x10000000000 as *const _).unwrap();
        let num_pages = (32 * 1024 * 1024 * 1024) >> PAGE_BITS;

        allocator.add_region(dram_base, num_pages).unwrap();

        assert_eq!(
            allocator
                .regions
                .iter()
                .map(|region| (**region).clone())
                .collect::<Vec<_>>(),
            vec![PhysicalMemoryRegion::new(dram_base, num_pages)]
        );
    }

    #[test]
    fn steal_regions() {
        let mut allocator = PhysicalPageAllocator::new();
        let dram_base = PhysicalAddress::try_from_ptr(0x10000000000 as *const _).unwrap();
        let num_pages = (32 * 1024 * 1024 * 1024) >> PAGE_BITS;

        allocator.add_region(dram_base, num_pages).unwrap();

        assert_eq!(
            allocator
                .regions
                .iter()
                .map(|region| (**region).clone())
                .collect::<Vec<_>>(),
            vec![PhysicalMemoryRegion::new(dram_base, num_pages)]
        );

        let region_base = PhysicalAddress::try_from_ptr(0x10000074000 as *const _).unwrap();
        let num_pages = 7;
        allocator.steal_region(region_base, num_pages).unwrap();

        let second_base = unsafe { PhysicalAddress::new_unchecked(0x10000090000 as *const _) };
        assert_eq!(
            allocator
                .regions
                .iter()
                .map(|region| (**region).clone())
                .collect::<Vec<_>>(),
            vec![
                PhysicalMemoryRegion::new(dram_base, 29),
                PhysicalMemoryRegion::new(second_base, 2097116),
            ]
        );

        let region_base = PhysicalAddress::try_from_ptr(0x10000090000 as *const _).unwrap();
        let num_pages = 9;
        allocator.steal_region(region_base, num_pages).unwrap();

        let region_base = PhysicalAddress::try_from_ptr(0x100000b4000 as *const _).unwrap();
        let num_pages = 46;
        allocator.steal_region(region_base, num_pages).unwrap();

        let region_base = PhysicalAddress::try_from_ptr(0x1000016c000 as *const _).unwrap();
        let num_pages = 262144;
        allocator.steal_region(region_base, num_pages).unwrap();

        let third_base = unsafe { PhysicalAddress::new_unchecked(0x1010016c000 as *const _) };
        assert_eq!(
            allocator
                .regions
                .iter()
                .map(|region| (**region).clone())
                .collect::<Vec<_>>(),
            vec![
                PhysicalMemoryRegion::new(dram_base, 29),
                PhysicalMemoryRegion::new(third_base, 1834917)
            ]
        );
    }
}
