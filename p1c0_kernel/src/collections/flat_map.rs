use crate::prelude::*;
use core::borrow::Borrow;

use core::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};
use core::mem::MaybeUninit;

// This is the default hasher. Currently uses a Crc32C hash
pub type FlatMapHasherBuilder = BuildHasherDefault<crate::hash::CrcHasher>;

type Result<T> = core::result::Result<T, Error>;

pub enum InsertStrategy {
    Replace,
    ErrorOnDuplicate,
}

#[derive(Debug)]
pub enum Error {
    KeyAlreadyPresentInMap,
    KeyNotFound,
}

enum BucketState {
    Empty,
    Deleted,
    InUse(u64),
}

struct Meta {
    hash: u64,
}

impl Default for Meta {
    fn default() -> Self {
        Meta::new()
    }
}

impl Meta {
    // 1 bit empty flag - 1 bit deleted - 56 bits hash
    const EMPTY_FLAG: u64 = 1 << 63;
    const DELETED_FLAG: u64 = 1 << 62;
    const HASH_MASK: u64 = !Self::EMPTY_FLAG;

    const fn new() -> Self {
        Meta {
            hash: Self::EMPTY_FLAG,
        }
    }

    #[must_use]
    fn is_bucket_empty(&self) -> bool {
        (self.hash & (Self::EMPTY_FLAG | Self::DELETED_FLAG)) == Self::EMPTY_FLAG
    }

    #[must_use]
    fn is_bucket_deleted(&self) -> bool {
        (self.hash & (Self::EMPTY_FLAG | Self::DELETED_FLAG))
            == (Self::EMPTY_FLAG | Self::DELETED_FLAG)
    }

    #[must_use]
    fn is_bucket_in_use(&self) -> bool {
        (self.hash & Self::EMPTY_FLAG) == 0
    }

    #[must_use]
    fn get_bucket_state(&self) -> BucketState {
        if self.is_bucket_in_use() {
            BucketState::InUse(self.hash & Self::HASH_MASK)
        } else if self.is_bucket_deleted() {
            BucketState::Deleted
        } else {
            BucketState::Empty
        }
    }

    fn set_in_use(&mut self, hash: u64) {
        self.hash = Self::HASH_MASK & hash;
    }

    fn set_deleted(&mut self) {
        self.hash = Self::EMPTY_FLAG | Self::DELETED_FLAG;
    }

    fn set_empty(&mut self) {
        self.hash = Self::EMPTY_FLAG;
    }

    #[must_use]
    fn get_hash(&self) -> Option<u64> {
        if self.is_bucket_in_use() {
            Some(self.hash & Self::HASH_MASK)
        } else {
            None
        }
    }

    #[must_use]
    fn matches_hash(&self, hash: u64) -> Option<bool> {
        if self.is_bucket_in_use() {
            Some((hash & Self::HASH_MASK) == self.hash)
        } else {
            None
        }
    }
}

pub struct FlatMap<K, V, H = FlatMapHasherBuilder>
where
    K: Hash + Eq + PartialEq,
    H: BuildHasher,
{
    /*
     * Keeping metadata as a contiguous allocation means that it has better chances on being
     * cache-efficient when traversing the tree.
     *
     * That is the reason for having the metadata buckets separate from the regular buckets, since
     * most of the time we are worried about traversing the metadata rather than looking up all
     * values.
     *
     * TODO(javier-varez): metadata and buckets could be allocated contiguously to reduce the number
     * of allocations upon resizing.
     */
    metadata_buckets: Vec<Meta>,
    buckets: Vec<MaybeUninit<(K, V)>>,
    num_elements: usize,
    capacity: usize,
    _hasher_builder: core::marker::PhantomData<H>,
}

impl<K, V> Default for FlatMap<K, V, FlatMapHasherBuilder>
where
    K: Hash + Eq + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FlatMap<K, V, FlatMapHasherBuilder>
where
    K: Hash + Eq + PartialEq,
{
    pub fn new() -> Self {
        Self::new_with_hasher(core::marker::PhantomData)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, core::marker::PhantomData)
    }
}

impl<K, V, H> FlatMap<K, V, H>
where
    K: Hash + Eq + PartialEq,
    H: BuildHasher,
{
    // 70% max load factor. If this is exceeded then we resize
    const MAX_LOAD_FACTOR: usize = 70;

    // Every time we resize we add RESIZE_FACTOR times more memory. This is designed to grow fast to
    // avoid too many resizes, so we reduce the number of reallocation at the cost of increasing
    // memory usage.
    const RESIZE_FACTOR: usize = 8;

    // Default capacity of the map when instantiated with ::new()
    const DEFAULT_CAPACITY: usize = 8;

    #[must_use]
    pub fn new_with_hasher(hasher_builder: core::marker::PhantomData<H>) -> Self {
        Self::with_capacity_and_hasher(Self::DEFAULT_CAPACITY, hasher_builder)
    }

    #[must_use]
    pub fn with_capacity_and_hasher(
        capacity: usize,
        hasher_builder: core::marker::PhantomData<H>,
    ) -> Self {
        let mut instance = Self {
            metadata_buckets: Vec::with_capacity(capacity),
            buckets: Vec::with_capacity(capacity),
            num_elements: 0,
            capacity,
            _hasher_builder: hasher_builder,
        };

        instance.metadata_buckets.resize_with(capacity, Meta::new);
        instance.buckets.resize_with(capacity, MaybeUninit::uninit);
        instance
    }

    fn hash_key<Q>(key: &Q) -> u64
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hasher_builder = FlatMapHasherBuilder::default();
        let mut hasher = hasher_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish()
    }

    #[must_use]
    fn rehash(hash: u64) -> u64 {
        let hasher_builder = FlatMapHasherBuilder::default();
        let mut hasher = hasher_builder.build_hasher();
        hash.hash(&mut hasher);
        hasher.finish()
    }

    /// Integer between 0-100 (%) to indicate the number of used entries / capacity of the table
    #[must_use]
    pub fn load_factor(&self) -> usize {
        (self.num_elements * 100) / self.capacity
    }

    fn resize(&mut self, new_capacity: usize) {
        let mut old_map = core::mem::replace(
            self,
            Self::with_capacity_and_hasher(new_capacity, core::marker::PhantomData),
        );

        for index in 0..old_map.capacity {
            if old_map.metadata_buckets[index].is_bucket_in_use() {
                // Since the metadata marks this as used we can get the index and value safely
                let (key, val) = unsafe {
                    core::mem::replace(&mut old_map.buckets[index], MaybeUninit::uninit())
                        .assume_init()
                };

                self.insert_without_resize(key, val, InsertStrategy::ErrorOnDuplicate)
                    .expect(concat!(
                    "Could not insert element when resizing! ",
                    "This must be a bug since the entry must fit and there cannot be a repeated key"
                    ));
            }
        }
    }

    fn insert_without_resize(&mut self, key: K, value: V, strategy: InsertStrategy) -> Result<()> {
        let key_hash = Self::hash_key(&key);

        let mut current_hash = key_hash;
        let mut found_deleted_slot = None;
        loop {
            let index = current_hash as usize % self.capacity;
            match self.metadata_buckets[index].get_bucket_state() {
                BucketState::Empty => {
                    let index = if let Some(deleted_slot_idx) = found_deleted_slot {
                        // The key was not found, but there was a deleted slot, so we should insert
                        // there instead of using the empty slot.
                        deleted_slot_idx
                    } else {
                        index
                    };

                    self.metadata_buckets[index].set_in_use(key_hash);
                    self.buckets[index].write((key, value));
                    self.num_elements += 1;
                    return Ok(());
                }
                BucketState::InUse(hash) if hash == (key_hash & Meta::HASH_MASK) => {
                    // The hash seems to match, lets double check hash collisions by checking if the
                    // keys do too. If so, we just replace the value in this slot
                    // (if the strategy is replace) or error out.

                    // # Safety: This is safe because we know the current bucket is in use
                    let (key_in_map, value_in_map) =
                        unsafe { self.buckets[index].assume_init_mut() };

                    if *key_in_map != key {
                        // This must be a hash collision, a rare ocasion, but it happens
                        current_hash = Self::rehash(current_hash);
                        continue;
                    }

                    match strategy {
                        InsertStrategy::ErrorOnDuplicate => {
                            return Err(Error::KeyAlreadyPresentInMap);
                        }
                        InsertStrategy::Replace => {
                            // Replace the old value
                            *value_in_map = value;
                            return Ok(());
                        }
                    }
                }
                BucketState::InUse(_) => {
                    // This bucket is used and does not match the hash, so we continue searching
                    // Rehash and try again
                    current_hash = Self::rehash(current_hash);
                }
                BucketState::Deleted => {
                    // Although we might be able to insert here, it would not work because the
                    // key might be already in further down the list. So we traverse the whole
                    // list until the end (empty bucket) and then if it is not present add it
                    // to one of the deleted slots (the first in the chain).
                    if found_deleted_slot.is_none() {
                        found_deleted_slot = Some(index);
                    }
                    current_hash = Self::rehash(current_hash);
                }
            }
        }
    }

    #[must_use]
    fn lookup_index<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let key_hash = Self::hash_key(key);

        let mut option = None;
        let mut current_hash = key_hash;
        loop {
            let index = current_hash as usize % self.capacity;
            match self.metadata_buckets[index].get_bucket_state() {
                BucketState::Empty => {
                    break;
                }
                BucketState::InUse(hash) if hash == (key_hash & Meta::HASH_MASK) => {
                    // # Safety: This is safe because we know the current bucket is in use
                    let (key_in_map, _) = unsafe { self.buckets[index].assume_init_ref() };

                    if *key_in_map.borrow() == *key {
                        option = Some(index);
                        break;
                    }

                    // This must be a hash collision, a rare ocasion, but it happens
                    current_hash = Self::rehash(current_hash);
                }
                BucketState::InUse(_) | BucketState::Deleted => {
                    // This bucket is used and does not match the hash, so we continue searching
                    // Rehash and try again
                    current_hash = Self::rehash(current_hash);
                }
            }
        }
        option
    }

    #[must_use]
    pub fn lookup<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.lookup_index(key).map(|index| {
            let (_k, v) = unsafe { self.buckets[index].assume_init_ref() };
            v
        })
    }

    #[must_use]
    pub fn lookup_mut<'a, Q>(&'a mut self, key: &'_ Q) -> Option<&'a mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.lookup_index(key).map(|index| {
            let (_k, v) = unsafe { self.buckets[index].assume_init_mut() };
            v
        })
    }

    pub fn insert(&mut self, key: K, value: V) {
        // This cannot error out because the insert strategy is replace
        self.insert_with_strategy(key, value, InsertStrategy::Replace)
            .unwrap()
    }

    pub fn insert_with_strategy(
        &mut self,
        key: K,
        value: V,
        strategy: InsertStrategy,
    ) -> Result<()> {
        if self.load_factor() > Self::MAX_LOAD_FACTOR {
            let new_capacity = self.capacity * Self::RESIZE_FACTOR;
            self.resize(new_capacity);
        }
        self.insert_without_resize(key, value, strategy)
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Result<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.lookup_index(key)
            .map(|index| {
                self.metadata_buckets[index].set_deleted();
                let element = core::mem::replace(&mut self.buckets[index], MaybeUninit::uninit());
                let (_k, v) = unsafe { element.assume_init() };
                v
            })
            .ok_or(Error::KeyNotFound)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.num_elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn test_can_create_map() {
        let _map: FlatMap<String, u32> = FlatMap::new();
    }

    #[test]
    fn test_can_create_map_with_capacity() {
        let _map: FlatMap<String, u32> = FlatMap::with_capacity(1024);
    }

    #[test]
    fn test_can_insert_elements() {
        let mut map = FlatMap::new();
        map.insert("Does this make sense?".to_string(), "cool!".to_string());
    }

    #[test]
    fn test_can_lookup_element() {
        let mut map = FlatMap::new();
        map.insert("Does this make sense?".to_string(), "cool!".to_string());

        let res = map.lookup("Does this make sense?");
        assert!(res.is_some());
        assert!(res.unwrap() == "cool!");

        let res = map.lookup("Does this make sense");
        assert!(res.is_none());
    }

    #[test]
    fn test_can_remove_elements() {
        let mut map = FlatMap::new();
        map.insert("Does this make sense?".to_string(), "cool!".to_string());
        map.insert("second key".to_string(), "nice!".to_string());

        let res = map.lookup("Does this make sense?");
        assert!(res.is_some());
        assert!(res.unwrap() == "cool!");

        let res = map.lookup("second key");
        assert!(res.is_some());
        assert!(res.unwrap() == "nice!");

        let res = map.remove("Does this make sense?");
        assert!(res.is_ok());
        assert!(res.unwrap() == "cool!");

        map.remove("Does this make sense?").unwrap_err();

        let res = map.lookup("Does this make sense?");
        assert!(res.is_none());

        let res = map.lookup("second key");
        assert!(res.is_some());
        assert!(res.unwrap() == "nice!");
    }

    #[test]
    fn test_automatically_resizes() {
        type StrFlatMap = FlatMap<String, String, FlatMapHasherBuilder>;
        let mut map = FlatMap::new();

        assert_eq!(map.capacity(), StrFlatMap::DEFAULT_CAPACITY);

        for i in 0..StrFlatMap::DEFAULT_CAPACITY {
            let key = format!("key {}", i);
            let value = format!("value {}", i);
            map.insert_with_strategy(key, value, InsertStrategy::ErrorOnDuplicate)
                .unwrap();
        }

        assert_eq!(
            map.capacity(),
            StrFlatMap::DEFAULT_CAPACITY * StrFlatMap::RESIZE_FACTOR
        );

        for i in 0..StrFlatMap::DEFAULT_CAPACITY {
            let key = format!("key {}", StrFlatMap::DEFAULT_CAPACITY + i);
            let value = format!("value {}", StrFlatMap::DEFAULT_CAPACITY + i);
            map.insert_with_strategy(key, value, InsertStrategy::ErrorOnDuplicate)
                .unwrap();
        }

        for i in 0..FlatMap::<String, String, FlatMapHasherBuilder>::DEFAULT_CAPACITY * 2 {
            let key = format!("key {}", i);
            let value = format!("value {}", i);
            assert_eq!(*map.lookup(&key).unwrap(), value);
        }
    }
}
