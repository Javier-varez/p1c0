use crate::{
    adt::get_adt,
    drivers::{generic_timer, interfaces::timer::Timer},
    memory::{address::Address, MemoryManager},
};

use core::{iter::Iterator, mem::MaybeUninit, time::Duration};

use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    register_bitfields,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

register_bitfields! {u32,
    Control [
        RUN OFFSET(0) NUMBITS(1) [],
        TX_RESET OFFSET(2) NUMBITS(1) [],
        RX_RESET OFFSET(3) NUMBITS(1) [],
    ],

    Config [
        CPHA OFFSET(1) NUMBITS(1) [],
        CPOL OFFSET(2) NUMBITS(1) [],
        MODE OFFSET(5) NUMBITS(2) [
            POLLED = 0,
            IRQ = 1,
            DMA = 2,
        ],
        IE_RXCOMPLETE OFFSET(7) NUMBITS(1) [],
        IE_TXRXTHRESH OFFSET(8) NUMBITS(1) [],
        LSB_FIRST OFFSET(13) NUMBITS(1) [],
        WORD_SIZE OFFSET(15) NUMBITS(2) [
            SZ8B = 0,
            SZ16B = 1,
            SZ32B = 2,
        ],
        FIFO_THRESH OFFSET(17) NUMBITS(2) [
            TH8B = 0,
            TH4B = 1,
            TH1B = 2,
        ],
        IE_TXCOMPLETE OFFSET(21) NUMBITS(1) [],
    ],
    Status [
        RX_COMPLETE OFFSET(0) NUMBITS(1) [],
        TXRX_THRESH OFFSET(1) NUMBITS(1) [],
        TX_COMPLETE OFFSET(22) NUMBITS(1) []
    ],

    Pin [
        KEEP_MOSI OFFSET(0) NUMBITS(1) [],
        CS OFFSET(1) NUMBITS(1) [
            ENABLE = 0,
            DISABLE = 1
        ],
    ],

    FifoStatus [
        TX_FULL OFFSET(4) NUMBITS(1) [],
        LEVEL_TX OFFSET(8) NUMBITS(8) [],
        RX_EMPTY OFFSET(20) NUMBITS(1) [],
        LEVEL_RX OFFSET(24) NUMBITS(8) [],
    ],

    InterruptEnableXfer [
        RX_COMPLETE OFFSET(0) NUMBITS(1) [],
        TX_COMPLETE OFFSET(1) NUMBITS(1) []
    ],

    InterruptFlagXfer [
        RX_COMPLETE OFFSET(0) NUMBITS(1) [],
        TX_COMPLETE OFFSET(1) NUMBITS(1) []
    ],

    InterruptEnableFifo [
        RX_THRESH OFFSET(4) NUMBITS(1) [],
        TX_THRESH OFFSET(5) NUMBITS(1) [],
        RX_FULL OFFSET(8) NUMBITS(1) [],
        TX_EMPTY OFFSET(9) NUMBITS(1) [],
        RX_UNDERRUN OFFSET(16) NUMBITS(1) [],
        TX_OVERFLOW OFFSET(17) NUMBITS(1) [],
    ],

    InterruptFlagFifo [
        RX_THRESH OFFSET(4) NUMBITS(1) [],
        TX_THRESH OFFSET(5) NUMBITS(1) [],
        RX_FULL OFFSET(8) NUMBITS(1) [],
        TX_EMPTY OFFSET(9) NUMBITS(1) [],
        RX_UNDERRUN OFFSET(16) NUMBITS(1) [],
        TX_OVERFLOW OFFSET(17) NUMBITS(1) [],
    ],

    ShiftConfig [
        CLOCK_ENABLE OFFSET(0) NUMBITS(1) [],
        CS_ENABLE OFFSET(1) NUMBITS(1) [],
        AND_CLOCK_DATA OFFSET(8) NUMBITS(1) [],
        CS_AS_DATA OFFSET(9) NUMBITS(1) [],
        TX_ENABLE OFFSET(10) NUMBITS(1) [],
        RX_ENABLE OFFSET(11) NUMBITS(1) [],
        BITS OFFSET(16) NUMBITS(6) [],
        OVERRIDE_CS OFFSET(24) NUMBITS(1) [],
    ],

    PinConfig [
        KEEP_CLK OFFSET(0) NUMBITS(1) [],
        KEEP_CS OFFSET(1) NUMBITS(1) [],
        KEEP_MOSI OFFSET(2) NUMBITS(1) [],
        CLK_IDLE_VAL OFFSET(8) NUMBITS(1) [],
        CS_IDLE_VAL OFFSET(9) NUMBITS(1) [],
        MOSI_IDLE_VAL OFFSET(10) NUMBITS(1) [],
    ],

    DelayPre [
        ENABLE OFFSET(0) NUMBITS(1) [],
        NO_INTERBYTE OFFSET(1) NUMBITS(1) [],
        SET_SCK OFFSET(4) NUMBITS(1) [],
        SET_MOSI OFFSET(6) NUMBITS(1) [],
        SCK_VAL OFFSET(8) NUMBITS(1) [],
        MOSI_VAL OFFSET(12) NUMBITS(1) [],
    ],

    DelayPost [
        ENABLE OFFSET(0) NUMBITS(1) [],
        NO_INTERBYTE OFFSET(1) NUMBITS(1) [],
        SET_SCK OFFSET(4) NUMBITS(1) [],
        SET_MOSI OFFSET(6) NUMBITS(1) [],
        SCK_VAL OFFSET(8) NUMBITS(1) [],
        MOSI_VAL OFFSET(12) NUMBITS(1) [],
    ]
}

const CLOCK_DIV_MAX: u32 = 0x7FF;
const PARENT_CLK_HZ: u128 = 24_000_000; // TODO(javier-varez): deduct this from the clock source in adt

const CLK_RATE_DEFAULT: Duration = Duration::from_micros(1);
// 1 Mhz
const CS_TO_CLK_DELAY_DEFAULT: Duration = Duration::from_micros(0);
const CLK_TO_CS_DELAY_DEFAULT: Duration = Duration::from_micros(0);
const CS_IDLE_DELAY_DEFAULT: Duration = Duration::from_micros(0);

const FIFO_DEPTH: u32 = 16;

#[repr(C)]
struct SpiRegisters {
    control: ReadWrite<u32, Control::Register>,
    // 0x00
    config: ReadWrite<u32, Config::Register>,
    // 0x04
    status: ReadWrite<u32, Status::Register>,
    // 0x08
    pin: ReadWrite<u32, Pin::Register>,
    // 0x0C
    tx_data: WriteOnly<u32>,
    // 0x10
    reserved_1: [u32; 3],
    // 0x14
    rx_data: ReadOnly<u32>,
    // 0x20
    reserved_2: [u32; 3],
    // 0x24
    clk_div: ReadWrite<u32>,
    // 0x30
    rx_count: ReadWrite<u32>,
    // 0x34
    word_delay: ReadWrite<u32>,
    // 0x38
    reserved_3: [u32; 4],
    // 0x3C
    tx_count: ReadWrite<u32>,
    // 0x4C
    reserved_4: [u32; 47],
    // 0x50
    fifo_status: ReadWrite<u32, FifoStatus::Register>,
    // 0x10C
    reserved_5: [u32; 8],
    // 0x110
    ie_xfer: ReadWrite<u32, InterruptEnableXfer::Register>,
    // 0x130
    if_xfer: ReadWrite<u32, InterruptFlagXfer::Register>,
    // 0x134
    ie_fifo: ReadWrite<u32, InterruptEnableFifo::Register>,
    // 0x138
    if_fifo: ReadWrite<u32, InterruptFlagFifo::Register>,
    // 0x13C
    reserved_6: [u32; 4],
    // 0x140
    shift_config: ReadWrite<u32, ShiftConfig::Register>,
    // 0x150
    pin_config: ReadWrite<u32, PinConfig::Register>,
    // 0x154
    reserved_7: [u32; 2],
    // 0x158
    delay_pre: ReadWrite<u32, DelayPre::Register>,
    // 0x160
    reserved_8: u32,
    // 0x164
    delay_post: ReadWrite<u32, DelayPost::Register>, // 0x168
}

fn pointer_alignment<T>(ptr: *const T) -> usize {
    let address = ptr as usize;
    if address & 0x07 == 0 {
        8
    } else if address & 0x03 == 0 {
        4
    } else if address & 0x01 == 0 {
        2
    } else {
        1
    }
}

/// This function checks the alignment and size of the slices to obtain the best fit
/// transaction size that does not result in UB
fn deduct_transaction_size(tx_data: &[u8], rx_data: &[MaybeUninit<u8>]) -> TransactionSize {
    let tx_alignment = pointer_alignment(tx_data.as_ptr());
    let rx_alignment = pointer_alignment(tx_data.as_ptr());
    if tx_alignment >= 4
        && rx_alignment >= 4
        && (tx_data.len() % 4 == 0)
        && (rx_data.len() % 4 == 0)
    {
        TransactionSize::Ts4b
    } else if tx_alignment >= 2
        && rx_alignment >= 2
        && (tx_data.len() % 2 == 0)
        && (rx_data.len() % 2 == 0)
    {
        TransactionSize::Ts2b
    } else {
        TransactionSize::Ts1b
    }
}

#[derive(Debug, Clone, Copy)]
enum TransactionSize {
    Ts1b,
    Ts2b,
    Ts4b,
}

#[derive(Debug, Clone)]
pub enum Error {
    AdtNodeNotFound,
    AdtNodeNotCompatible,
    RxUnderrun,
    TxOverflow,
}

pub struct Spi {
    regs: &'static mut SpiRegisters,
    cs_to_clock_delay: Duration,
    clock_to_cs_delay: Duration,
    cs_inactive_delay: Duration,
    clock_rate: Duration,
}

impl Spi {
    /// Constructs a new Spi peripheral from the given adt node reference.
    ///
    /// # Safety
    /// The spi_node must not already be in use by any other piece of code.
    pub unsafe fn new(spi_node: &str) -> Result<Self, Error> {
        // TODO(javier-varez): Find out better alternatives that don't require unsafe when getting a
        // peripheral. This needs to be globally managed.
        let adt = get_adt().unwrap();
        let node = adt.find_node(spi_node).ok_or(Error::AdtNodeNotFound)?;

        if !node.is_compatible("spi-1,spimc") {
            return Err(Error::AdtNodeNotCompatible);
        }

        let (pa, _) = adt.get_device_addr(spi_node, 0).unwrap();

        let va = MemoryManager::instance()
            .map_io(spi_node, pa, core::mem::size_of::<SpiRegisters>())
            .expect("The spi device io cannot be mapped");

        let regs: &'static mut SpiRegisters = &mut *(va.as_ptr() as *mut SpiRegisters);

        let cs_to_clock_delay = CS_TO_CLK_DELAY_DEFAULT;
        let clock_to_cs_delay = CLK_TO_CS_DELAY_DEFAULT;
        let cs_inactive_delay = CS_IDLE_DELAY_DEFAULT;
        let clock_rate = CLK_RATE_DEFAULT;

        let mut instance = Self {
            regs,
            cs_to_clock_delay,
            clock_to_cs_delay,
            cs_inactive_delay,
            clock_rate,
        };

        instance.init();

        Ok(instance)
    }

    pub fn init(&mut self) {
        // Reset the RX and TX fifos
        self.regs
            .control
            .write(Control::TX_RESET::SET + Control::RX_RESET::SET);

        // Disable CS pin for now
        self.regs.pin.modify(Pin::CS::DISABLE);
        self.regs
            .shift_config
            .modify(ShiftConfig::OVERRIDE_CS::CLEAR);
        self.regs
            .pin_config
            .modify(PinConfig::CS_IDLE_VAL::CLEAR + PinConfig::KEEP_CS::SET);

        // This driver does not use IRQs for now given that AIC bringup is not done
        self.regs.ie_xfer.write(
            InterruptEnableXfer::TX_COMPLETE::CLEAR + InterruptEnableXfer::RX_COMPLETE::CLEAR,
        );

        self.regs.ie_fifo.write(
            InterruptEnableFifo::RX_FULL::CLEAR
                + InterruptEnableFifo::TX_EMPTY::CLEAR
                + InterruptEnableFifo::RX_THRESH::CLEAR
                + InterruptEnableFifo::TX_THRESH::CLEAR
                + InterruptEnableFifo::RX_UNDERRUN::CLEAR
                + InterruptEnableFifo::TX_OVERFLOW::CLEAR,
        );

        // Disable delays
        self.regs.delay_pre.write(DelayPre::ENABLE::CLEAR);
        self.regs.delay_post.write(DelayPost::ENABLE::CLEAR);

        // Set default configuration. For now we don't expose controls externally for these
        // settings. We may need to do that in the future, though.
        self.regs.config.write(
            Config::CPOL::CLEAR
                + Config::CPHA::CLEAR
                + Config::MODE::POLLED
                // SPI is normally MSB first. We probably won't even need to set this bit ever
                + Config::LSB_FIRST::CLEAR
                + Config::WORD_SIZE::SZ8B
                + Config::FIFO_THRESH::TH8B
                + Config::IE_TXRXTHRESH::CLEAR
                + Config::IE_RXCOMPLETE::CLEAR
                + Config::IE_TXCOMPLETE::CLEAR,
        );
    }

    fn set_cs(&mut self, enable: bool) {
        let field = if enable {
            Pin::CS::ENABLE
        } else {
            Pin::CS::DISABLE
        };
        self.regs.pin.modify(field);
    }

    /// # Safety
    /// The transaction size must match the number of elements in the iter. That is, if size of
    /// transaction is 4 bytes then the iter must have a number of valid elements multiple of 4.
    unsafe fn push_tx<'a, T>(&mut self, tx_data_iter: &'_ mut T, ts_size: TransactionSize)
    where
        T: Iterator<Item = &'a u8>,
    {
        let word_count = FIFO_DEPTH - self.regs.fifo_status.read(FifoStatus::LEVEL_TX);
        for _ in 0..word_count {
            if let Some(first_byte) = tx_data_iter.next() {
                match ts_size {
                    TransactionSize::Ts1b => {
                        let value = *first_byte;
                        self.regs.tx_data.set(value.into());
                    }
                    TransactionSize::Ts2b => {
                        let bytes = [*first_byte, *tx_data_iter.next().unwrap_unchecked()];
                        let value = u16::from_be_bytes(bytes);
                        self.regs.tx_data.set(value.into());
                    }
                    TransactionSize::Ts4b => {
                        let bytes = [
                            *first_byte,
                            *tx_data_iter.next().unwrap_unchecked(),
                            *tx_data_iter.next().unwrap_unchecked(),
                            *tx_data_iter.next().unwrap_unchecked(),
                        ];
                        let value = u32::from_be_bytes(bytes);
                        self.regs.tx_data.set(value);
                    }
                }
            } else {
                // Nothing to do.
                return;
            }
        }
    }

    /// # Safety
    /// The transaction size must match the number of elements in the iter. That is, if size of
    /// transaction is 4 bytes then the iter must have a number of valid elements multiple of 4.
    unsafe fn pop_rx<'a, T>(&mut self, rx_data_iter: &'_ mut T, ts_size: TransactionSize)
    where
        T: Iterator<Item = &'a mut MaybeUninit<u8>>,
    {
        while self.regs.fifo_status.read(FifoStatus::LEVEL_RX) > 0 {
            let rx_data = self.regs.rx_data.get();

            match ts_size {
                TransactionSize::Ts1b => {
                    let slot = rx_data_iter.next().unwrap_unchecked();
                    slot.write(rx_data as u8);
                }
                TransactionSize::Ts2b => {
                    let bytes = u16::to_be_bytes(rx_data as u16);
                    for byte in bytes {
                        let slot = rx_data_iter.next().unwrap_unchecked();
                        slot.write(byte);
                    }
                }
                TransactionSize::Ts4b => {
                    let bytes = u32::to_be_bytes(rx_data);
                    for byte in bytes {
                        let slot = rx_data_iter.next().unwrap_unchecked();
                        slot.write(byte);
                    }
                }
            }
        }
    }

    fn poll_for_errors(&self) -> Result<(), Error> {
        let rx_underrun = self.regs.if_fifo.read(InterruptFlagFifo::RX_UNDERRUN) != 0;
        let tx_overflow = self.regs.if_fifo.read(InterruptFlagFifo::TX_OVERFLOW) != 0;

        if rx_underrun {
            return Err(Error::RxUnderrun);
        }
        if tx_overflow {
            return Err(Error::TxOverflow);
        }

        Ok(())
    }

    fn poll_completion(&self, tx_len: usize, rx_len: usize) -> Result<(), Error> {
        if tx_len != 0 && rx_len != 0 {
            while self.regs.status.read(Status::TX_COMPLETE) == 0
                || self.regs.status.read(Status::RX_COMPLETE) == 0
            {
                self.poll_for_errors()?;
            }
        } else if tx_len != 0 {
            while self.regs.status.read(Status::TX_COMPLETE) == 0 {
                self.poll_for_errors()?;
            }
        } else if rx_len != 0 {
            while self.regs.status.read(Status::RX_COMPLETE) == 0 {
                self.poll_for_errors()?;
            }
        }
        Ok(())
    }

    pub fn transact(&mut self, tx_data: &[u8], rx_data: &mut [u8]) -> Result<(), Error> {
        // We know that the data is initialized. Faking as if it wasn't allows us to freely write
        // to it. Since u8 does not implement drop, no problems should arise from the objects not
        // being dropped with write
        let rx_data = unsafe { core::mem::transmute(rx_data) };
        self.transact_into_uninit_buffer(tx_data, rx_data)
    }

    pub fn set_cs_to_clock_delay(&mut self, duration: Duration) {
        self.cs_to_clock_delay = duration;
    }

    pub fn set_clock_to_cs_delay(&mut self, duration: Duration) {
        self.clock_to_cs_delay = duration;
    }

    pub fn set_cs_inactive_delay(&mut self, duration: Duration) {
        self.cs_inactive_delay = duration;
    }

    pub fn set_clock_rate(&mut self, duration: Duration) {
        self.clock_rate = duration;
    }

    pub fn transact_into_uninit_buffer(
        &mut self,
        tx_data: &[u8],
        rx_data: &mut [MaybeUninit<u8>],
    ) -> Result<(), Error> {
        if tx_data.is_empty() && rx_data.is_empty() {
            // This is effectively a noop
            return Ok(());
        }

        let ts_size = deduct_transaction_size(tx_data, rx_data);

        let (tx_len, rx_len) = match ts_size {
            TransactionSize::Ts1b => {
                self.regs.config.write(Config::WORD_SIZE::SZ8B);
                (tx_data.len(), rx_data.len())
            }
            TransactionSize::Ts2b => {
                self.regs.config.write(Config::WORD_SIZE::SZ16B);
                let bytes_per_transaction = core::mem::size_of::<u16>();
                (
                    tx_data.len() / bytes_per_transaction,
                    rx_data.len() / bytes_per_transaction,
                )
            }
            TransactionSize::Ts4b => {
                self.regs.config.write(Config::WORD_SIZE::SZ32B);
                let bytes_per_transaction = core::mem::size_of::<u32>();
                (
                    tx_data.len() / bytes_per_transaction,
                    rx_data.len() / bytes_per_transaction,
                )
            }
        };

        // Clear status registers
        self.regs.status.set(0xFFFFFFFF);
        self.regs.if_fifo.set(0xFFFFFFFF);
        self.regs.if_xfer.set(0xFFFFFFFF);

        self.regs.rx_count.set(rx_len as u32);
        self.regs.tx_count.set(tx_len as u32);

        let clk_div = (PARENT_CLK_HZ * self.clock_rate.as_nanos() / 1_000_000_000) as u32 - 1;
        self.regs
            .clk_div
            .set(core::cmp::min(clk_div, CLOCK_DIV_MAX));

        let mut tx_data_iter = tx_data.iter().peekable();
        let mut rx_data_iter = rx_data.iter_mut().peekable();
        unsafe {
            self.push_tx(&mut tx_data_iter, ts_size);
        }

        let timer = generic_timer::get_timer();

        let clock_to_cs_delay = self.clock_to_cs_delay;
        let cs_to_clock_delay = self.cs_to_clock_delay;
        let cs_inactive_delay = self.cs_inactive_delay;

        // Start the transfer
        self.set_cs(true);
        // TODO(javier-varez): maybe we should allow sleeping here?
        timer.delay(cs_to_clock_delay);
        self.regs.control.write(Control::RUN::SET);

        let cleanup = |instance: &mut Self| {
            // TODO(javier-varez): maybe we should allow sleeping here?
            timer.delay(clock_to_cs_delay);
            instance.set_cs(false);
            instance
                .regs
                .control
                .write(Control::RUN::CLEAR + Control::RX_RESET::SET + Control::TX_RESET::SET);
            // TODO(javier-varez): maybe we should allow sleeping here?
            timer.delay(cs_inactive_delay);
        };

        while tx_data_iter.peek().is_some() || rx_data_iter.peek().is_some() {
            unsafe {
                self.push_tx(&mut tx_data_iter, ts_size);
                self.pop_rx(&mut rx_data_iter, ts_size);
            }

            self.poll_for_errors().map_err(|err| {
                cleanup(self);
                err
            })?;
        }

        self.poll_completion(tx_len, rx_len).map_err(|err| {
            cleanup(self);
            err
        })?;

        cleanup(self);
        Ok(())
    }
}
