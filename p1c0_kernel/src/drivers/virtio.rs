mod input;
mod virtqueue;

use crate::{
    adt,
    memory::{self, address::Address, MemoryManager},
    prelude::*,
    sync::spinlock::RwSpinLock,
};

use p1c0_macros::initcall;

use tock_registers::{
    interfaces::Readable,
    register_bitfields,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

const COMPATIBLE: &str = "virtio,mmio";

#[derive(Debug)]
pub enum Error {
    AdtNotAvailable(adt::Error),
    NodeNotCompatible,
    MissingAdtProperty(&'static str),
    MemoryError(memory::Error),
    InvalidMagic,
    UnsupportedVersion(u32),
    UnsupportedDeviceId,
    UnknownDeviceId,
    DummyDevice,
    InitializationError,
    InvalidFeatures,
    DeviceSpecificError,
    EmptyDev,
}

impl From<memory::Error> for Error {
    fn from(err: memory::Error) -> Self {
        Self::MemoryError(err)
    }
}

trait Subdev {}

impl super::Device for Virtio {}

pub struct Virtio {
    _subdev: Box<dyn Subdev>,
}

impl Virtio {
    const MAGIC_VALUE: u32 = 0x74726976;
    const SUPPORTED_VERSION: u32 = 2;

    pub fn probe(path: &[adt::AdtNode]) -> Result<Self, Error> {
        let adt = adt::get_adt().map_err(Error::AdtNotAvailable)?;

        let node = path.last().expect("No path given!");
        if !node.is_compatible(COMPATIBLE) {
            return Err(Error::NodeNotCompatible);
        }

        let (pa, size) = adt
            .get_device_addr_from_nodes(path, 0)
            .ok_or(Error::MissingAdtProperty("reg"))?;

        let base_address = MemoryManager::instance().map_io(node.get_name(), pa, size)?;
        let regs: &'static VirtioMmioRegs::Bank =
            unsafe { &*(base_address.as_ptr() as *const VirtioMmioRegs::Bank) };

        if regs.magic.get() != Self::MAGIC_VALUE {
            log_warning!("virtio device block does not contain magic value");
            return Err(Error::InvalidMagic);
        }

        let version = regs.version.get();
        if version != Self::SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let subdev = match regs.device_id.read_as_enum(DeviceId::ID) {
            Some(DeviceId::ID::Value::Input) => {
                log_debug!("Found input device!");
                Box::new(input::InputSubdevice::probe(regs)?)
            }
            Some(DeviceId::ID::Value::Dummy) => {
                log_debug!("Unused virtio,mmio. Dummy device found");
                return Err(Error::EmptyDev);
            }
            Some(_) => {
                log_warning!("Other virtio device ids are unsupported");
                return Err(Error::UnsupportedDeviceId);
            }
            None => {
                return Err(Error::UnknownDeviceId);
            }
        };

        log_debug!("Probe ok!");

        Ok(Virtio { _subdev: subdev })
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl From<Error> for super::Error {
    fn from(e: Error) -> Self {
        super::Error::DeviceSpecificError(Box::new(e))
    }
}

#[initcall(priority = 0)]
fn virtio_register_driver() {
    super::register_driver(COMPATIBLE, Box::new(VirtioDriver {})).unwrap();
}

struct VirtioDriver {}

impl super::Driver for VirtioDriver {
    fn probe(&self, dev_path: &[adt::AdtNode]) -> super::Result<super::DeviceRef> {
        let dev = Virtio::probe(dev_path)?;
        Ok(Arc::new(RwSpinLock::new(dev)))
    }
}

register_bitfields! {u32,
    DeviceId [
        ID OFFSET(0) NUMBITS(32) [
            Dummy = 0,
            Network = 1,
            Block = 2,
            Console = 3,
            Entropy = 4,
            MemoryBalloon = 5,
            SCSIHost = 8,
            GPU = 16,
            Input = 18,
            Socket = 19,
            Crypto = 20
        ]
    ],

    DeviceStatus [
        ACK OFFSET(0) NUMBITS(1) [],
        DRIVER OFFSET(1) NUMBITS(1) [],
        DRIVER_OK OFFSET(2) NUMBITS(1) [],
        FEATURES_OK OFFSET(3) NUMBITS(1) [],
        DEVICE_NEEDS_RESET OFFSET(6) NUMBITS(1) [],
        FAILED OFFSET(7) NUMBITS(1) []
    ],

    Interrupt [
        USED_BUFFER_NOTIFICATION OFFSET(0) NUMBITS(1) [],
        CONFIGURATION_CHANGE_NOTIFICATION OFFSET(1) NUMBITS(1) [],
    ],

    FeatureBits1 [
        /// Negotiating this feature indicates that the driver can use descriptors with the
        /// VIRTQ_DESC_F_INDIRECT flag set,
        RING_INDIRECT_DESC OFFSET(28) NUMBITS(1) [],
        /// This feature enables the used_event and the avail_event fields in the virtqueues
        RING_EVENT_IDX OFFSET(29) NUMBITS(1) [],
    ],
    FeatureBits2 [
        /// This indicates compliance with this specification, giving a simple way to detect legacy
        /// devices or drivers
        VERSION_1 OFFSET(0) NUMBITS(1) [],
        /// This feature indicates that the device can be used on a platform where device access to
        /// data in memory is limited and/or translated. E.g. this is the case if the device can be
        /// located behind an IOMMU that translates bus addresses from the device into physical
        /// addresses in memory, if the device can be limited to only access certain memory
        /// addresses or if special commands such as a cache flush can be needed to synchronise data
        /// in memory with the device. Whether accesses are actually limited or translated is
        /// described by platform-specific means. If this feature bit is set to 0, then the device
        /// has same access to memory addresses supplied to it as the driver has. In particular,
        /// the device will always use physical addresses matching addresses used by the driver
        /// (typically meaning physical addresses used by the CPU) and not translated further, and
        /// can access any address supplied to it by the driver. When clear, this overrides any
        /// platform-specific description of whether device access is limited or translated in any
        /// way, e.g. whether an IOMMU may be present.
        ACCESS_PLATFORM OFFSET(1) NUMBITS(1) [],
        /// This feature indicates support for the packed virtqueue layout
        RING_PACKED OFFSET(2) NUMBITS(1) [],
        /// This feature indicates that all buffers are used by the device in the same order in
        /// which they have been made available.
        IN_ORDER OFFSET(3) NUMBITS(1) [],
        /// This feature indicates that memory accesses by the driver and the device are ordered in
        /// a way described by the platform.
        ///
        /// If this feature bit is negotiated, the ordering in effect for any memory accesses by the
        /// driver that need to be ordered in a specific way with respect to accesses by the device
        /// is the one suitable for devices described by the platform. This implies that the driver
        /// needs to use memory barriers suitable for devices described by the platform; e.g. for the
        /// PCI transport in the case of hardware PCI devices.
        ///
        /// If this feature bit is not negotiated, then the device and driver are assumed to be
        /// implemented in software, that is they can be assumed to run on identical CPUs in an SMP
        /// configuration. Thus a weaker form of memory barriers is sufficient to yield better
        /// performance.
        ORDER_PLATFORM OFFSET(4) NUMBITS(1) [],
        /// This feature indicates that the device supports Single Root I/O Virtualization.
        /// Currently only PCI devices support this feature.
        SR_IOV OFFSET(5) NUMBITS(1) [],
        /// This feature indicates that the driver passes extra data
        /// (besides identifying the virtqueue) in its device notifications.
        NOTIFICATION_DATA OFFSET(6) NUMBITS(1) [],
    ]
}

p1c0_macros::define_register_bank! {
    VirtioMmioRegs<4> {
        <0x00> => magic: ReadOnly<u32>,
        <0x04> => version: ReadOnly<u32>,
        <0x08> => device_id: ReadOnly<u32, DeviceId::Register>,
        <0x0c> => vendor_id: ReadOnly<u32>,
        <0x10> => device_features: ReadOnly<u32>,
        <0x14> => device_features_sel: WriteOnly<u32>,
        <0x20> => driver_features: WriteOnly<u32>,
        <0x24> => driver_features_sel: WriteOnly<u32>,
        <0x30> => queue_sel: WriteOnly<u32>,
        <0x34> => queue_num_max: ReadOnly<u32>,
        <0x38> => queue_num: WriteOnly<u32>,
        <0x44> => queue_ready: ReadWrite<u32>,
        <0x50> => queue_notify: WriteOnly<u32>,
        <0x60> => interrupt_status: ReadOnly<u32, Interrupt::Register>,
        <0x64> => interrupt_ack: WriteOnly<u32, Interrupt::Register>,
        <0x70> => status: ReadWrite<u32, DeviceStatus::Register>,
        <0x80> => queue_descriptor_low: WriteOnly<u32>,
        <0x84> => queue_descriptor_high: WriteOnly<u32>,
        <0x90> => queue_driver_low: WriteOnly<u32>,
        <0x94> => queue_driver_high: WriteOnly<u32>,
        <0xa0> => queue_device_low: WriteOnly<u32>,
        <0xa4> => queue_device_high: WriteOnly<u32>,
        <0xfc> => config_generation: ReadOnly<u32>,
    }
}
