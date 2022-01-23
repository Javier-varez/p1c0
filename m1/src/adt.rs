#[derive(Debug, Clone)]
pub enum Error {
    InvalidAlignment,
    InvalidNodeHeader,
    InvalidPropertyHeader,
    UnknownNode,
    InvalidPropertyType,
    InvalidRangeDataSize,
    InvalidRegDataSize,
}

use core::ops::FnMut;
use core::{mem, slice, str};

use heapless::Vec;

/// ADT Memory layout
///
/// There is no header for the ADT. At offset 0 the first node can be found.
///
/// A node header has 2 elements in this order:
///   * N number of properties: 4-bytes le integer
///   * M number of children: 4-byte le integer
///
/// After the node header, the following N elements corerspond to the properties of the node. Each
/// property has the following structure:
///   * property name [32 bytes], null terminated
///   * size of the value: 4-byte le integer
///   * value of the property. The type of the property is not encoded into the ADT and therefore
///     needs to be known by the user beforehand (given by the name) in order to parse it adequately.
///
/// After the properties for the current node, there are M children, which follow the same
/// structure.

#[repr(C)]
struct AdtNodeHeader {
    num_properties: u32,
    num_children: u32,
}

#[repr(C)]
struct AdtPropertyHeader {
    name: [u8; 32],
    value_size: u32,
}

#[derive(Debug, Clone)]
pub struct Adt {
    head: AdtNode,
}

#[derive(Debug, Clone)]
pub struct AdtNode {
    header: *const AdtNodeHeader,
}

impl AdtNode {
    /// # Safety
    ///   `ptr` must point to the beginning of the ADT and contain valid ADT data. The ADT data
    ///   must be valid for the duration of the program ('static)
    unsafe fn new(ptr: *const u8) -> Result<Self, Error> {
        if ptr.align_offset(mem::size_of::<u32>()) != 0 {
            return Err(Error::InvalidAlignment);
        }

        let node_header = &*(ptr as *const AdtNodeHeader);
        if node_header.num_properties == 0 {
            return Err(Error::InvalidNodeHeader);
        }

        Ok(AdtNode {
            header: node_header as *const _,
        })
    }

    fn first_child_ptr(&self) -> *const u8 {
        self.property_iter()
            .last()
            .map(|prop| prop.end_ptr())
            .expect("All nodes should have at least 1 property")
    }

    fn first_property_ptr(&self) -> *const u8 {
        let node_base = self.header as *const u8;
        unsafe { node_base.add(mem::size_of::<AdtNodeHeader>()) }
    }

    pub fn child_iter(&self) -> NodeIter {
        NodeIter {
            curr_ptr: self.first_child_ptr(),
            num_nodes: unsafe { (*self.header).num_children },
        }
    }

    pub fn find_child(&self, name: &str) -> Option<AdtNode> {
        self.child_iter().find(|child| child.get_name() == name)
    }

    pub fn get_name(&self) -> &'static str {
        self.find_property("name")
            .expect("All nodes have a name property")
            .str_value()
            .expect("The content of the \"name\" property is a valid utf8 str")
    }

    pub fn property_iter(&self) -> PropertyIter {
        PropertyIter {
            curr_ptr: self.first_property_ptr(),
            num_properties: unsafe { (*self.header).num_properties },
        }
    }

    pub fn find_property(&self, name: &str) -> Option<AdtProperty> {
        self.property_iter()
            .find(|property| property.get_name() == name)
    }

    pub fn get_address_cells(&self) -> Option<u32> {
        self.find_property("#address-cells").and_then(|prop| {
            prop.u32_value()
                .map(|val| {
                    debug_assert!(val <= 2 && val > 0);
                    val
                })
                .ok()
        })
    }

    pub fn get_size_cells(&self) -> Option<u32> {
        self.find_property("#size-cells").and_then(|prop| {
            prop.u32_value()
                .map(|val| {
                    debug_assert!(val <= 2);
                    val
                })
                .ok()
        })
    }

    pub fn range_iter(&self, parent_address_cells: Option<u32>) -> AdtRangeIter {
        let size_cells = self.get_size_cells().unwrap_or(2);
        let address_cells = self.get_address_cells().unwrap_or(2);
        let parent_address_cells = parent_address_cells.unwrap_or(2);
        let data = self
            .find_property("ranges")
            .map(|prop| prop.get_data())
            .unwrap_or(&[]);
        AdtRangeIter::new(data, address_cells, size_cells, parent_address_cells)
    }

    pub fn reg_iter(
        &self,
        parent_address_cells: Option<u32>,
        parent_size_cells: Option<u32>,
    ) -> AdtRegIter {
        let parent_address_cells = parent_address_cells.unwrap_or(2);
        let parent_size_cells = parent_size_cells.unwrap_or(2);
        let data = self
            .find_property("reg")
            .map(|prop| prop.get_data())
            .unwrap_or(&[]);
        AdtRegIter::new(data, parent_address_cells, parent_size_cells)
    }

    fn end_ptr(&self) -> *const u8 {
        // Try to get the end ptr from the last child (recursively). If there are no childs this is the exit
        // condition and we return the start of what would be the first child
        self.child_iter()
            .last()
            .map(|node| node.end_ptr())
            .unwrap_or_else(|| {
                // There are no children, so the beginning of the children is already the next node
                self.first_child_ptr()
            })
    }
}

macro_rules! define_value_method {
    ($func_name: ident, $type: ty) => {
        pub fn $func_name(&self) -> Result<$type, Error> {
            const SIZE: usize = core::mem::size_of::<$type>();
            if self.get_size() < SIZE {
                return Err($crate::adt::Error::InvalidPropertyType);
            }

            let data = &self.get_data()[..SIZE];
            let bytes: [u8; SIZE] = data.try_into().expect("There are exactly SIZE elements");
            Ok(<$type>::from_le_bytes(bytes))
        }
    };
}

#[derive(Debug, Clone)]
pub struct AdtProperty {
    header: *const AdtPropertyHeader,
}

impl AdtProperty {
    /// # Safety
    ///   `ptr` must point to the beginning of the ADT and contain valid ADT data. The ADT data
    ///   must be valid for the duration of the program ('static)
    unsafe fn new(ptr: *const u8) -> Result<Self, Error> {
        if ptr.align_offset(mem::size_of::<u32>()) != 0 {
            return Err(Error::InvalidAlignment);
        }

        const MAX_SIZE: u32 = 1024 * 1024 * 1024; // 1MB prop
        let prop_header = &*(ptr as *const AdtPropertyHeader);
        if prop_header.value_size > MAX_SIZE {
            return Err(Error::InvalidPropertyHeader);
        }

        Ok(AdtProperty {
            header: prop_header as *const _,
        })
    }

    fn end_ptr(&self) -> *const u8 {
        let prop_start = self.header as *const u8;

        unsafe {
            let end = prop_start
                .add(mem::size_of::<AdtPropertyHeader>())
                .add((*self.header).value_size as usize);

            // Align the end of the ptr to a 32 bit boundary as required by the ADT spec
            let alingment = end.align_offset(mem::size_of::<u32>());
            end.add(alingment)
        }
    }

    pub fn get_name(&self) -> &'static str {
        let prop_name_data = unsafe { &(*self.header).name };
        let prop_name_data = prop_name_data
            .split(|val| *val == b'\0')
            .next()
            .expect("At least one null character is present in the string");
        str::from_utf8(prop_name_data).expect("Only UTF8 values in the string")
    }

    pub fn get_size(&self) -> usize {
        unsafe { (*self.header).value_size as usize }
    }

    pub fn str_value(&self) -> Result<&'static str, Error> {
        let prop_data = self
            .get_data()
            .split(|val| *val == b'\0')
            .next()
            .ok_or(Error::InvalidPropertyType)?;
        str::from_utf8(prop_data).map_err(|_| Error::InvalidPropertyType)
    }

    pub fn str_list_value(&self) -> StrListIter<impl FnMut(&'_ u8) -> bool> {
        StrListIter {
            inner_iter: self.get_data().split(|byte| *byte == b'\0'),
        }
    }

    define_value_method!(u8_value, u8);
    define_value_method!(u16_value, u16);
    define_value_method!(u32_value, u32);
    define_value_method!(u64_value, u64);
    define_value_method!(usize_value, usize);

    define_value_method!(i8_value, i8);
    define_value_method!(i16_value, i16);
    define_value_method!(i32_value, i32);
    define_value_method!(i64_value, i64);
    define_value_method!(isize_value, isize);

    /// Returns a slice with the data contained in value.
    pub fn get_data(&self) -> &'static [u8] {
        unsafe {
            let data_ptr = self.header.add(1) as *const u8;
            let data_size = (*self.header).value_size;
            slice::from_raw_parts(data_ptr, data_size.try_into().unwrap())
        }
    }
}

impl Adt {
    /// # Safety
    ///   `ptr` must point to the beginning of the ADT and contain valid ADT data. The ADT data
    ///   must be valid for the duration of the program ('static)
    pub unsafe fn new(ptr: *const u8) -> Result<Self, Error> {
        let head = AdtNode::new(ptr)?;
        Ok(Adt { head })
    }

    pub fn find_node(&self, path: &str) -> Option<AdtNode> {
        if path == "/" {
            return Some(self.head.clone());
        }

        // Remove leading and trailing slashes. After this the only slashes present should separate
        // the node hierarchy levels
        let path = path.trim_matches('/');

        let mut node = self.head.clone();
        for node_name in path.split('/') {
            node = node.find_child(node_name)?;
        }
        Some(node)
    }

    pub fn path_iter<'a>(&self, path: &'a str) -> PathIter<'a> {
        PathIter {
            node: self.head.clone(),
            path,
        }
    }

    pub fn get_device_addr(&self, path: &str, reg_index: usize) -> Option<(usize, usize)> {
        let nodes: Vec<AdtNode, 8> = self.path_iter(path).collect();

        let mut iter = nodes.iter().rev();
        let mut child = iter.next()?;
        let mut maybe_parent = iter.clone().next();
        let pa_cells = maybe_parent.and_then(|node| node.get_address_cells());
        let ps_cells = maybe_parent.and_then(|node| node.get_size_cells());

        let reg = child.reg_iter(pa_cells, ps_cells).nth(reg_index)?;

        let mut addr = reg.get_addr();
        let size = reg.get_size();

        for node in iter {
            child = maybe_parent.unwrap();
            maybe_parent = Some(node);

            let pa_cells = maybe_parent.and_then(|node| node.get_address_cells());

            child.range_iter(pa_cells).for_each(|range| {
                // Only use those in the region
                if (addr >= range.get_bus_addr())
                    && ((addr + size) < (range.get_bus_addr() + range.get_size()))
                {
                    addr += range.get_parent_addr() - range.get_bus_addr();
                }
            });
        }

        Some((addr, size))
    }
}

#[derive(Debug, Clone)]
pub struct NodeIter {
    num_nodes: u32,
    curr_ptr: *const u8,
}

impl Iterator for NodeIter {
    type Item = AdtNode;
    fn next(&mut self) -> Option<Self::Item> {
        if self.num_nodes > 0 {
            let node = unsafe {
                AdtNode::new(self.curr_ptr)
                    .expect("Should be a valid pointer. Otherwise there is an implementation bug")
            };
            self.num_nodes -= 1;
            self.curr_ptr = node.end_ptr();
            Some(node)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct PropertyIter {
    num_properties: u32,
    curr_ptr: *const u8,
}

impl Iterator for PropertyIter {
    type Item = AdtProperty;
    fn next(&mut self) -> Option<Self::Item> {
        if self.num_properties > 0 {
            let property = unsafe {
                AdtProperty::new(self.curr_ptr)
                    .expect("Should be a valid pointer. Otherwise there is an implementation bug")
            };
            self.num_properties -= 1;
            self.curr_ptr = property.end_ptr();
            Some(property)
        } else {
            None
        }
    }
}

pub fn get_adt() -> Result<Adt, Error> {
    let boot_args = crate::boot_args::get_boot_args();
    unsafe {
        let addr = boot_args
            .device_tree
            .offset(-(boot_args.virt_base as isize))
            .add(boot_args.phys_base);
        Adt::new(addr)
    }
}

pub struct StrListIter<P>
where
    P: FnMut(&u8) -> bool,
{
    inner_iter: slice::Split<'static, u8, P>,
}

impl<P> Iterator for StrListIter<P>
where
    P: FnMut(&'_ u8) -> bool,
{
    type Item = &'static str;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter
            .next()
            .and_then(|data| str::from_utf8(data).ok())
            .and_then(|str| if str.is_empty() { None } else { Some(str) })
    }
}

/// Sizes for the AdtRange might be different from u32
///   * sizeof::<bus_addr>() = child.address_cells
///   * sizeof::<parent_addr>() = parent.address_cells
///   * sizeof::<size>() = child.address_cells
///
#[derive(Clone, Debug)]
pub struct AdtRange {
    data: &'static [u8],
    address_cells: u32,
    parent_address_cells: u32,
    size_cells: u32,
}

impl AdtRange {
    fn new(
        data: &'static [u8],
        address_cells: u32,
        size_cells: u32,
        parent_address_cells: u32,
    ) -> Result<Self, Error> {
        let entry_size = core::mem::size_of::<u32>()
            * (address_cells + size_cells + parent_address_cells) as usize;
        if data.len() != entry_size {
            return Err(Error::InvalidRangeDataSize);
        }

        Ok(Self {
            data,
            address_cells,
            parent_address_cells,
            size_cells,
        })
    }

    fn bus_addr_offset(&self) -> usize {
        0
    }

    fn parent_addr_offset(&self) -> usize {
        self.address_cells as usize * core::mem::size_of::<u32>()
    }

    fn size_offset(&self) -> usize {
        self.address_cells as usize * core::mem::size_of::<u32>()
            + self.parent_address_cells as usize * core::mem::size_of::<u32>()
    }

    pub fn get_bus_addr(&self) -> usize {
        let offset = self.bus_addr_offset();
        match self.address_cells {
            1 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<u32>()];
                let bytes: [u8; core::mem::size_of::<u32>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                u32::from_le_bytes(bytes) as usize
            }
            2 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<usize>()];
                let bytes: [u8; core::mem::size_of::<usize>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                usize::from_le_bytes(bytes)
            }
            _ => unimplemented!(),
        }
    }

    pub fn get_parent_addr(&self) -> usize {
        let offset = self.parent_addr_offset();
        match self.parent_address_cells {
            1 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<u32>()];
                let bytes: [u8; core::mem::size_of::<u32>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                u32::from_le_bytes(bytes) as usize
            }
            2 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<usize>()];
                let bytes: [u8; core::mem::size_of::<usize>()] =
                    borrow.try_into().expect("There are exactly 8 elements");
                usize::from_le_bytes(bytes)
            }
            _ => unimplemented!(),
        }
    }

    pub fn get_size(&self) -> usize {
        let offset = self.size_offset();
        match self.size_cells {
            1 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<u32>()];
                let bytes: [u8; core::mem::size_of::<u32>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                u32::from_le_bytes(bytes) as usize
            }
            2 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<usize>()];
                let bytes: [u8; core::mem::size_of::<usize>()] =
                    borrow.try_into().expect("There are exactly 8 elements");
                usize::from_le_bytes(bytes)
            }
            _ => unimplemented!(),
        }
    }
}

impl core::fmt::Display for AdtRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Range [bus address 0x{:x}, parent address 0x{:x}, size 0x{:x}]",
            self.get_bus_addr(),
            self.get_parent_addr(),
            self.get_size()
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AdtRangeIter {
    data: &'static [u8],
    address_cells: u32,
    parent_address_cells: u32,
    size_cells: u32,
}

impl AdtRangeIter {
    fn new(
        data: &'static [u8],
        address_cells: u32,
        size_cells: u32,
        parent_address_cells: u32,
    ) -> Self {
        let entry_size = core::mem::size_of::<u32>()
            * (address_cells + size_cells + parent_address_cells) as usize;
        let data = if (data.len() % entry_size) == 0 {
            data
        } else {
            &[]
        };

        Self {
            data,
            address_cells,
            parent_address_cells,
            size_cells,
        }
    }

    fn get_entry_size(&self) -> usize {
        core::mem::size_of::<u32>()
            * (self.address_cells + self.size_cells + self.parent_address_cells) as usize
    }
}

impl Iterator for AdtRangeIter {
    type Item = AdtRange;
    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            let (next, new_data) = self.data.split_at(self.get_entry_size());
            self.data = new_data;
            Some(
                AdtRange::new(
                    next,
                    self.address_cells,
                    self.size_cells,
                    self.parent_address_cells,
                )
                .ok()?,
            )
        }
    }
}

/// Sizes for the AdtRange might be different from u32
///   * sizeof::<bus_addr>() = child.address_cells
///   * sizeof::<parent_addr>() = parent.address_cells
///   * sizeof::<size>() = child.address_cells
///
#[derive(Clone, Debug)]
pub struct AdtReg {
    data: &'static [u8],
    parent_address_cells: u32,
    parent_size_cells: u32,
}

impl AdtReg {
    fn new(
        data: &'static [u8],
        parent_address_cells: u32,
        parent_size_cells: u32,
    ) -> Result<Self, Error> {
        let entry_size =
            core::mem::size_of::<u32>() * (parent_address_cells + parent_size_cells) as usize;
        if data.len() != entry_size {
            return Err(Error::InvalidRangeDataSize);
        }

        Ok(Self {
            data,
            parent_address_cells,
            parent_size_cells,
        })
    }

    fn addr_offset(&self) -> usize {
        0
    }

    fn size_offset(&self) -> usize {
        self.parent_address_cells as usize * core::mem::size_of::<u32>()
    }

    pub fn get_addr(&self) -> usize {
        let offset = self.addr_offset();
        match self.parent_address_cells {
            1 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<u32>()];
                let bytes: [u8; core::mem::size_of::<u32>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                u32::from_le_bytes(bytes) as usize
            }
            2 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<usize>()];
                let bytes: [u8; core::mem::size_of::<usize>()] =
                    borrow.try_into().expect("There are exactly 8 elements");
                usize::from_le_bytes(bytes)
            }
            _ => unimplemented!(),
        }
    }

    pub fn get_size(&self) -> usize {
        let offset = self.size_offset();
        match self.parent_size_cells {
            1 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<u32>()];
                let bytes: [u8; core::mem::size_of::<u32>()] =
                    borrow.try_into().expect("There are exactly 4 elements");
                u32::from_le_bytes(bytes) as usize
            }
            2 => {
                let borrow = &self.data[offset..offset + core::mem::size_of::<usize>()];
                let bytes: [u8; core::mem::size_of::<usize>()] =
                    borrow.try_into().expect("There are exactly 8 elements");
                usize::from_le_bytes(bytes)
            }
            _ => unimplemented!(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AdtRegIter {
    data: &'static [u8],
    parent_address_cells: u32,
    parent_size_cells: u32,
}

impl AdtRegIter {
    fn new(data: &'static [u8], parent_address_cells: u32, parent_size_cells: u32) -> Self {
        let entry_size =
            core::mem::size_of::<u32>() * (parent_size_cells + parent_address_cells) as usize;
        let data = if (data.len() % entry_size) == 0 {
            data
        } else {
            &[]
        };

        Self {
            data,
            parent_address_cells,
            parent_size_cells,
        }
    }

    fn get_entry_size(&self) -> usize {
        core::mem::size_of::<u32>() * (self.parent_size_cells + self.parent_address_cells) as usize
    }
}

impl Iterator for AdtRegIter {
    type Item = AdtReg;
    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            None
        } else {
            let (next, new_data) = self.data.split_at(self.get_entry_size());
            self.data = new_data;
            Some(AdtReg::new(next, self.parent_address_cells, self.parent_size_cells).ok()?)
        }
    }
}

pub struct PathIter<'a> {
    node: AdtNode,
    path: &'a str,
}

impl<'a> Iterator for PathIter<'a> {
    type Item = AdtNode;
    fn next(&mut self) -> Option<Self::Item> {
        if self.path == "/" || self.path.is_empty() {
            return None;
        }

        let path = self.path.trim_start_matches('/');

        let mut splits = path.splitn(2, '/');
        let node_name = splits.next().unwrap();
        let node_or_none = self.node.find_child(node_name);
        if node_or_none.is_none() {
            self.path = "";
            return None;
        }

        let node = node_or_none.unwrap();

        if let Some(remaining_path) = splits.next() {
            self.path = remaining_path;
        } else {
            self.path = "";
        }
        self.node = node.clone();

        Some(node)
    }
}
