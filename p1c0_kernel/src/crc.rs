mod crc16 {
    const POLY: u16 = 0xA001; // x^16 + x^15 + x^2 + 1

    const fn generate_coefficient(byte: u8) -> u16 {
        let mut value: u16 = byte as u16;

        let mut i = 0;
        while i < 8 {
            if (0x1 & value) != 0 {
                value >>= 1;
                value ^= POLY;
            } else {
                value >>= 1;
            }

            i += 1;
        }

        value
    }

    const fn generate_table() -> [u16; 256] {
        let mut table = [0; 256];
        let mut i = 0;
        while i < 256 {
            table[i] = generate_coefficient(i as u8);
            i += 1;
        }
        table
    }

    pub(super) static TABLE: [u16; 256] = generate_table();

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_generated_table() {
            assert_eq!(TABLE[0], 0);
            assert_eq!(TABLE[1], 0xC0C1);
            assert_eq!(TABLE[2], 0xC181);
            assert_eq!(TABLE[3], 0x0140);
            assert_eq!(TABLE[255], 0x4040);
        }
    }
}

pub fn crc16(seed: u16, data: &[u8]) -> u16 {
    let mut crc16: u16 = seed;
    for byte in data {
        let index = *byte ^ (crc16 & 0xff) as u8;
        crc16 = (crc16 >> 8) ^ crc16::TABLE[index as usize];
    }
    crc16
}

mod crc32c {
    const POLY: u32 = 0x82F63B78; // CRC32C

    const fn generate_coefficient(byte: u8) -> u32 {
        let mut value = byte as u32;

        let mut i = 0;
        while i < 8 {
            if (0x1 & value) != 0 {
                value >>= 1;
                value ^= POLY;
            } else {
                value >>= 1;
            }

            i += 1;
        }

        value
    }

    const fn generate_table() -> [u32; 256] {
        let mut table = [0; 256];
        let mut i = 0;
        while i < 256 {
            table[i] = generate_coefficient(i as u8);
            i += 1;
        }
        table
    }

    pub(super) static TABLE: [u32; 256] = generate_table();
}

pub struct Crc32C {
    current_value: u32,
}

impl Crc32C {
    const INITIAL_VALUE: u32 = 0xFFFFFFFF;
    const XOR_OUT: u32 = 0xFFFFFFFF;

    pub const fn new() -> Self {
        Self {
            current_value: Self::INITIAL_VALUE,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let index = *byte ^ (self.current_value & 0xff) as u8;
            self.current_value = (self.current_value >> 8) ^ crc32c::TABLE[index as usize];
        }
    }

    pub fn finish(&self) -> u32 {
        self.current_value ^ Self::XOR_OUT
    }
}

pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc32c = Crc32C::new();
    crc32c.write(data);
    crc32c.finish()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_crc16() {
        assert_eq!(crc16(0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]), 0x96c5);

        assert_eq!(
            crc16(0, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0xc5, 0x96]),
            0x0
        );
    }

    #[test]
    fn test_crc16_with_seed() {
        let initial = crc16(0, &[0, 1, 2]);
        let final_val = crc16(initial, &[3, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_eq!(final_val, 0x96c5);

        assert_eq!(
            crc16(
                final_val,
                &[(final_val & 0xff) as u8, (final_val >> 8) as u8]
            ),
            0
        );
    }

    #[test]
    fn test_crc32c() {
        assert_eq!(crc32c(&[0x00]), 0x527D5351);
        assert_eq!(crc32c(&[0x00, 0x01, 0x02, 0xA5]), 0x5DD948ED);
        assert_eq!(crc32c(&[0x12, 0x23, 0x4F, 0xFF]), 0xA01D7DB4);
    }
}
