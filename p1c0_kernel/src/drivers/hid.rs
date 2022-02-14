pub mod keyboard;

use super::generic_timer;
use super::gpio::{self, GpioBank, PinState};
use super::spi::{self, Spi};

use crate::adt;

use core::{mem::MaybeUninit, time::Duration};

use keyboard::{Keyboard, KeyboardReport};

#[derive(Debug)]
pub enum IoError {
    CannotRequestGpio,
    SpiError(spi::Error),
    InvalidCRC,
}

#[derive(Debug)]
pub enum Error {
    NodeNotFound,
    NodeNotCompatible,
    ProbeFailed,
    InvalidAdt(adt::Error),
    IOError(IoError),
}

const KBD_DEVICE_ID: u8 = 1;
const TRACKPAD_DEVICE_ID: u8 = 2;

#[repr(C)]
#[derive(Debug)]
struct HidTransferPacket {
    flags: u8,
    device: u8,
    offset: u16,
    remaining: u16,
    length: u16,
    data: [u8; 246],
    crc16: u16,
}

#[repr(C)]
#[derive(Debug)]
struct HidMsgHeader {
    byte0: u8,
    byte1: u8,
    byte2: u8,
    id: u8,
    rsplen: u16,
    len: u16,
}

impl From<spi::Error> for Error {
    fn from(error: spi::Error) -> Self {
        Error::IOError(IoError::SpiError(error))
    }
}

pub struct HidDev<'a> {
    spidev: Spi,
    enable_pin: gpio::Pin<'a, gpio::mode::Output>,
    irq_pin: gpio::Pin<'a, gpio::mode::Input>,
    keyboard_dev: Keyboard,
}

impl<'a> HidDev<'a> {
    // TODO(javier-varez): Don't take ownership of the devices, since multiple devices might be
    // on the same bus. Need to figure out a device good ownership model for this.

    /// # Safety
    /// The hid device must not have been instantiated before. The spidev and gpio_bank must match
    /// the spi used for the hid device and the enable gpio pin used. Hopefully in the future a device
    /// framework will abstract all this...
    ///
    pub unsafe fn new(
        hid_name: &str,
        mut spidev: Spi,
        gpio0_bank: &'a GpioBank,
        nub_gpio0_bank: &'a GpioBank,
    ) -> Result<Self, Error> {
        let adt = adt::get_adt().map_err(Error::InvalidAdt)?;

        let hid_node = adt.find_node(hid_name).ok_or(Error::NodeNotFound)?;
        if !hid_node.is_compatible("hid-transport,spi") {
            return Err(Error::NodeNotCompatible);
        }

        let spi_en_function = hid_node
            .find_property("function-spi_en")
            .and_then(|property| property.function_value().ok())
            .ok_or(Error::ProbeFailed)?;
        let enable_pin = spi_en_function.args[0] as usize;

        let enable_pin = gpio0_bank
            .request_as_output(enable_pin, PinState::Low)
            .or(Err(Error::IOError(IoError::CannotRequestGpio)))?;

        let irq_pin_num = hid_node
            .find_property("interrupts")
            .and_then(|property| property.u32_value().ok())
            .ok_or(Error::ProbeFailed)?;

        let irq_pin = nub_gpio0_bank
            .request_as_input(irq_pin_num as usize)
            .or(Err(Error::IOError(IoError::CannotRequestGpio)))?;

        spidev.set_cs_inactive_delay(Duration::from_micros(250));
        spidev.set_cs_to_clock_delay(Duration::from_micros(45));
        spidev.set_clock_to_cs_delay(Duration::from_micros(45));
        spidev.set_clock_rate(Duration::from_nanos(125)); // 1 / 8 MHz

        Ok(Self {
            spidev,
            enable_pin,
            irq_pin,
            keyboard_dev: Keyboard::new(),
        })
    }

    pub fn wait_for_irq(&mut self) {
        loop {
            if let PinState::Low = self.irq_pin.get_pin_state() {
                break;
            }
        }
    }

    pub fn power_on(&mut self) {
        let timer = generic_timer::get_timer();

        self.enable_pin.set_pin_state(PinState::High);
        timer.delay(Duration::from_millis(5));

        self.enable_pin.set_pin_state(PinState::Low);
        timer.delay(Duration::from_millis(5));

        self.enable_pin.set_pin_state(PinState::High);
        timer.delay(Duration::from_millis(50));
    }

    pub fn power_off(&mut self) {
        self.enable_pin.set_pin_state(PinState::High);
    }

    fn receive_packet(&mut self) -> Result<HidTransferPacket, Error> {
        let mut hid_packet: MaybeUninit<HidTransferPacket> = MaybeUninit::uninit();
        let packet_bytes = hid_packet.as_bytes_mut();

        self.spidev.transact_into_uninit_buffer(&[], packet_bytes)?;

        // At this point, the bytes are initialized after the transaction
        let packet_bytes = unsafe { MaybeUninit::slice_assume_init_ref(packet_bytes) };

        let crc = crate::crc::crc16(0, &packet_bytes);
        if crc != 0 {
            crate::println!("Invalid CRC from hid device {}", crc);
            return Err(Error::IOError(IoError::InvalidCRC));
        }

        return Ok(unsafe { hid_packet.assume_init() });
    }

    fn parse_keyboard_packet(&mut self, packet: HidTransferPacket) {
        if packet.length as usize >= core::mem::size_of::<HidMsgHeader>() {
            let header: HidMsgHeader = unsafe { core::mem::transmute_copy(&packet.data) };
            if header.len >= 9
                && header.byte0 == 0x10
                && header.byte1 == 0x01
                && header.byte2 == 0x00
            {
                let off = core::mem::size_of::<HidMsgHeader>();
                let data = &packet.data[off..off + header.len as usize];

                let report = KeyboardReport::new(data);
                self.keyboard_dev.handle_report(report);
            }
        } else {
            crate::println!("Short keyboard packet");
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.wait_for_irq();

            let packet = self.receive_packet().unwrap();
            match packet.device {
                KBD_DEVICE_ID => {
                    self.parse_keyboard_packet(packet);
                }
                TRACKPAD_DEVICE_ID => {
                    // Ignore trackpad packets for now
                    // crate::println!("Trackpad packet, {:?}", packet);
                }
                _ => {
                    crate::println!("Unknown packet, {:?}", packet);
                }
            }
        }
    }
}
