use crate::crc::Crc32C;

use core::hash::Hasher;

// This is not a cryptographically safe hasher, but it is easy to implement and works well enough.
pub struct CrcHasher {
    crc32c: Crc32C,
}

impl Default for CrcHasher {
    fn default() -> Self {
        CrcHasher {
            crc32c: Crc32C::new(),
        }
    }
}

impl Hasher for CrcHasher {
    fn finish(&self) -> u64 {
        self.crc32c.finish() as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        self.crc32c.write(bytes);
    }
}
