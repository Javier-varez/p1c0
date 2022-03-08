/// Single-producer, single-consumer buffer that allows the user to share data across threads
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Debug)]
pub enum Error {
    AlreadySplit,
    WouldBlock,
}

#[derive(Debug)]
pub struct RingBuffer<const SIZE: usize> {
    data: [UnsafeCell<MaybeUninit<u8>>; SIZE],
    read_index: AtomicUsize,
    write_index: AtomicUsize,
    is_split: AtomicBool,
}

/// # Safety
/// The RingBuffer can only be accessed by the Writer/Reader, so even though it is an unsafe cell it
/// still isn't accessed directly via the ring buffer, but rather through the Writer/Reader
unsafe impl<const SIZE: usize> Sync for RingBuffer<SIZE> {}

impl<const SIZE: usize> RingBuffer<SIZE> {
    pub const fn new() -> Self {
        #[allow(clippy::declare_interior_mutable_const)]
        const CELL: UnsafeCell<MaybeUninit<u8>> = UnsafeCell::new(MaybeUninit::uninit());
        Self {
            data: [CELL; SIZE],
            read_index: AtomicUsize::new(0),
            write_index: AtomicUsize::new(0),
            is_split: AtomicBool::new(false),
        }
    }

    // Not exposed to the public API because it doesn't seem useful outside of the context of the
    // implementation
    fn free_space(&self) -> usize {
        let read_index = self.read_index.load(Ordering::Relaxed);
        let write_index = self.write_index.load(Ordering::Relaxed);
        // We subtract one in both cases to not have to deal with the case where the queue is
        // completely full, which is usually indicated by a boolean
        if read_index <= write_index {
            read_index + SIZE - write_index - 1
        } else {
            read_index - write_index - 1
        }
    }

    fn fill_level(&self) -> usize {
        let read_index = self.read_index.load(Ordering::Relaxed);
        let write_index = self.write_index.load(Ordering::Relaxed);
        if read_index <= write_index {
            write_index - read_index
        } else {
            write_index + SIZE - read_index
        }
    }

    fn increment_index(index_atomic: &AtomicUsize, ordering: Ordering) {
        loop {
            let prev_index = index_atomic.load(Ordering::Relaxed);
            let mut index = prev_index + 1;
            if index >= SIZE {
                index = 0;
            }

            if index_atomic
                .compare_exchange_weak(prev_index, index, ordering, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    fn push(&self, data: u8) -> Result<(), Error> {
        if self.free_space() < 1 {
            return Err(Error::WouldBlock);
        }

        let write_index = self.write_index.load(Ordering::Relaxed);

        // # Safety:
        //   This should be safe because since the reader can only read it cannot reserve memory
        //   from the data buffer. We already verified that there is enough space, so writing to the
        //   next element is safe as long as there is only one writer
        unsafe {
            *self.data[write_index].get() = MaybeUninit::new(data);
        }

        // Finally increment the index and publish the element by using the correct memory ordering.
        Self::increment_index(&self.write_index, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Result<u8, Error> {
        if self.fill_level() < 1 {
            return Err(Error::WouldBlock);
        }

        let read_index = self.read_index.load(Ordering::Relaxed);

        // # Safety:
        //   This should be safe because since the writer cannot write to memory used by the reader
        //   from the data buffer. We already verified that there is enough data, so reading from
        //   the buffer the next element is safe as long as there is only one reader
        let data = unsafe { (*self.data[read_index].get()).assume_init() };
        Self::increment_index(&self.read_index, Ordering::Acquire);
        Ok(data)
    }

    pub fn split(&self) -> Result<(Writer<'_, SIZE>, Reader<'_, SIZE>), Error> {
        if !self.is_split.swap(true, Ordering::Relaxed) {
            Ok((Writer { buffer: self }, Reader { buffer: self }))
        } else {
            Err(Error::AlreadySplit)
        }
    }
}

#[derive(Debug)]
pub struct Writer<'a, const SIZE: usize> {
    buffer: &'a RingBuffer<SIZE>,
}

#[derive(Debug)]
pub struct Reader<'a, const SIZE: usize> {
    buffer: &'a RingBuffer<SIZE>,
}

impl<'a, const SIZE: usize> Writer<'a, SIZE> {
    pub fn push(&mut self, data: u8) -> Result<(), Error> {
        self.buffer.push(data)
    }
}

impl<'a, const SIZE: usize> Reader<'a, SIZE> {
    pub fn pop(&mut self) -> Result<u8, Error> {
        self.buffer.pop()
    }
}

/// # Safety
/// It is safe to send the Reader across threads because even though it has a shared reference to
/// the array of UnsafeCell, it is guaranteed that the Reader and Writer won't mutate the same cell
/// at the same time, since there is clear ownership between them
unsafe impl<'a, const SIZE: usize> Send for Reader<'a, SIZE> {}

/// # Safety
/// It is safe to send the Writer across threads because even though it has a shared reference to
/// the array of UnsafeCell, it is guaranteed that the Reader and Writer won't mutate the same cell
/// at the same time, since there is clear ownership between them
unsafe impl<'a, const SIZE: usize> Send for Writer<'a, SIZE> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_can_split_the_buffer() {
        let ring_buffer: RingBuffer<16> = RingBuffer::new();
        let (_writer, _reader) = ring_buffer.split().unwrap();
    }

    #[test]
    fn test_more_than_one_split_fails() {
        let ring_buffer: RingBuffer<16> = RingBuffer::new();
        let (_writer, _reader) = ring_buffer.split().unwrap();
        ring_buffer.split().unwrap_err();
    }

    #[test]
    fn test_empty_buffer_does_not_pop() {
        let ring_buffer: RingBuffer<16> = RingBuffer::new();
        let (_writer, mut reader) = ring_buffer.split().unwrap();

        assert!(matches!(reader.pop(), Err(Error::WouldBlock)));
    }

    #[test]
    fn test_can_push_to_the_buffer() {
        let ring_buffer: RingBuffer<16> = RingBuffer::new();
        let (mut writer, _reader) = ring_buffer.split().unwrap();

        writer.push(123).unwrap();
    }

    #[test]
    fn test_can_pop_from_the_buffer() {
        let ring_buffer: RingBuffer<16> = RingBuffer::new();
        let (mut writer, mut reader) = ring_buffer.split().unwrap();

        writer.push(123).unwrap();
        assert_eq!(reader.pop().unwrap(), 123);
        assert!(matches!(reader.pop(), Err(Error::WouldBlock)));
    }

    #[test]
    fn test_cannot_push_more_than_its_size() {
        let ring_buffer: RingBuffer<4> = RingBuffer::new();
        let (mut writer, _reader) = ring_buffer.split().unwrap();

        writer.push(0).unwrap();
        writer.push(1).unwrap();
        writer.push(2).unwrap();
        assert!(matches!(writer.push(3), Err(Error::WouldBlock)));
    }

    #[test]
    fn test_can_read_as_many_elements_as_pushed() {
        let ring_buffer: RingBuffer<4> = RingBuffer::new();
        let (mut writer, mut reader) = ring_buffer.split().unwrap();

        writer.push(0).unwrap();
        writer.push(1).unwrap();
        writer.push(2).unwrap();
        assert!(matches!(writer.push(3), Err(Error::WouldBlock)));

        assert_eq!(reader.pop().unwrap(), 0);
        assert_eq!(reader.pop().unwrap(), 1);
        assert_eq!(reader.pop().unwrap(), 2);
        assert!(matches!(reader.pop(), Err(Error::WouldBlock)));
    }

    #[test]
    fn test_can_keep_pushing_after_reading() {
        let ring_buffer: RingBuffer<4> = RingBuffer::new();
        let (mut writer, mut reader) = ring_buffer.split().unwrap();

        writer.push(0).unwrap();
        writer.push(1).unwrap();
        writer.push(2).unwrap();
        assert!(matches!(writer.push(3), Err(Error::WouldBlock)));

        assert_eq!(reader.pop().unwrap(), 0);

        writer.push(3).unwrap();
        assert!(matches!(writer.push(4), Err(Error::WouldBlock)));

        assert_eq!(reader.pop().unwrap(), 1);
        assert_eq!(reader.pop().unwrap(), 2);
        assert_eq!(reader.pop().unwrap(), 3);
        assert!(matches!(reader.pop(), Err(Error::WouldBlock)));
    }

    #[test]
    fn test_works_across_threads() {
        let ring_buffer: RingBuffer<4> = RingBuffer::new();
        let (mut writer, mut reader) = ring_buffer.split().unwrap();

        std::thread::scope(|s| {
            s.spawn(|_| {
                for i in 0..16 {
                    writer.push(i).unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            });

            s.spawn(|_| {
                for i in 0..16 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    assert_eq!(reader.pop().unwrap(), i);
                }
            });
        });
    }
}
