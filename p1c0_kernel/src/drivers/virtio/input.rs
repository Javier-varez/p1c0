use crate::{
    memory::address::Address,
    prelude::*,
    thread::{self, ThreadHandle},
};

use super::{
    virtqueue::VirtQueue, DeviceStatus, FeatureBits1, FeatureBits2, Subdev, VirtioMmioRegs,
};

use crate::sync::spinlock::SpinLock;
use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    registers::InMemoryRegister,
};

#[allow(clippy::enum_variant_names)]
pub enum Error {
    InvalidEventType(u16),
    InvalidKey(u16),
    InvalidKeyState(u32),
}

pub struct InputSubdev {
    _thread_handle: ThreadHandle,
}

struct InputSubdevImpl {
    regs: &'static VirtioMmioRegs::Bank,
    eventq: InputVirtQueue,
    _statusq: InputVirtQueue,
}

const EVENTQ_IDX: u32 = 0;
const STATUSQ_IDX: u32 = 1;
const QUEUE_SIZE: usize = 16;
const DESC_BUFFER_SIZE: usize = 32;

type InputVirtQueue = VirtQueue<QUEUE_SIZE, DESC_BUFFER_SIZE>;

impl InputSubdev {
    pub fn probe(regs: &'static VirtioMmioRegs::Bank) -> Result<Self, super::Error> {
        regs.status.modify(DeviceStatus::ACK::SET);
        regs.status.modify(DeviceStatus::DRIVER::SET);

        if let Err(e) = Self::negotiate_feature_bits(regs) {
            regs.status.modify(DeviceStatus::FAILED::SET);
            return Err(e);
        }

        // These devices have 2 virqueues which need to be configured here
        let (mut eventq, mut statusq) = match Self::allocate_and_configure_virtqueues(regs) {
            Ok(res) => res,
            Err(e) => {
                regs.status.modify(DeviceStatus::FAILED::SET);
                return Err(e);
            }
        };

        for _ in 0..QUEUE_SIZE {
            eventq.post_event();
            statusq.post_event();
        }
        regs.queue_notify.set(EVENTQ_IDX);
        regs.queue_notify.set(STATUSQ_IDX);

        // Finally go live!
        regs.status.modify(DeviceStatus::DRIVER_OK::SET);
        let instance = SpinLock::new(InputSubdevImpl {
            regs,
            eventq,
            _statusq: statusq,
        });

        // Instead of using IRQs, a primitive poll handler is used here... Not great!
        let thread_handle = thread::spawn(move || loop {
            {
                'inner: loop {
                    let mut instance = instance.lock();
                    if instance
                        .regs
                        .interrupt_status
                        .read(super::Interrupt::USED_BUFFER_NOTIFICATION)
                        == 0
                    {
                        break 'inner;
                    }
                    instance
                        .regs
                        .interrupt_ack
                        .write(super::Interrupt::USED_BUFFER_NOTIFICATION::SET);

                    instance.eventq.handle_events(|data| {
                        let event_type = u16::from_le_bytes([data[0], data[1]]);
                        let event_type: EventType = match event_type.try_into() {
                            Ok(EventType::Key) => EventType::Key,
                            Ok(EventType::Sync) => {
                                // We ignore sync events
                                return;
                            }
                            Ok(event_type) => {
                                log_warning!("Ignored event type {:?}", event_type);
                                return;
                            }
                            Err(_) => {
                                log_warning!("Invalid event type {}", event_type);
                                return;
                            }
                        };

                        let key_type = u16::from_le_bytes([data[2], data[3]]);
                        let key_type: Keys = match key_type.try_into() {
                            Ok(val) => val,
                            Err(_) => {
                                log_warning!("Invalid key type {}", key_type);
                                return;
                            }
                        };
                        let key_state = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                        let key_state: KeyState = match key_state.try_into() {
                            Ok(val) => val,
                            Err(_) => {
                                log_warning!("Invalid key state {}", key_state);
                                return;
                            }
                        };

                        let event = Event {
                            _ty: event_type,
                            _key: key_type,
                            _state: key_state,
                        };

                        log_debug!("User pressed {:?}", event);
                    });

                    if instance.eventq.should_notify() {
                        instance.regs.queue_notify.set(EVENTQ_IDX);
                    }
                }
            }
            crate::syscall::Syscall::yield_exec();
        });

        Ok(Self {
            _thread_handle: thread_handle,
        })
    }

    fn negotiate_feature_bits(regs: &'static VirtioMmioRegs::Bank) -> Result<(), super::Error> {
        // Read feature bits, word 1
        regs.device_features_sel.set(0);
        let feature_bits_1: InMemoryRegister<u32, FeatureBits1::Register> =
            InMemoryRegister::new(regs.device_features.get());

        log_verbose!("Feature bits word 1: 0x{:08x}", feature_bits_1.get());
        if feature_bits_1.read(FeatureBits1::RING_EVENT_IDX) == 0 {
            log_warning!("Ring event index not supported!");
            return Err(super::Error::InvalidFeatures);
        }

        // Read feature bits, word 2
        regs.device_features_sel.set(1);
        let feature_bits_2: InMemoryRegister<u32, FeatureBits2::Register> =
            InMemoryRegister::new(regs.device_features.get());

        log_verbose!("Feature bits word 2: 0x{:08x}", feature_bits_2.get());
        if feature_bits_2.read(FeatureBits2::VERSION_1) == 0 {
            log_warning!("Unsupported version");
            return Err(super::Error::InvalidFeatures);
        }

        // Accept just required features
        feature_bits_1.write(FeatureBits1::RING_EVENT_IDX::CLEAR);
        feature_bits_2.write(FeatureBits2::VERSION_1::SET);

        regs.driver_features_sel.set(0);
        regs.driver_features.set(feature_bits_1.get());
        regs.driver_features_sel.set(1);
        regs.driver_features.set(feature_bits_2.get());

        regs.status.modify(DeviceStatus::FEATURES_OK::SET);

        if regs.status.read(DeviceStatus::FEATURES_OK) == 0 {
            log_warning!("Unsupported subset of features");
            return Err(super::Error::InvalidFeatures);
        }

        log_verbose!("Features OK!");
        Ok(())
    }

    fn allocate_and_configure_virtqueues(
        regs: &'static VirtioMmioRegs::Bank,
    ) -> Result<(InputVirtQueue, InputVirtQueue), super::Error> {
        let eventq = InputVirtQueue::allocate();
        let statusq = InputVirtQueue::allocate();

        regs.queue_sel.set(EVENTQ_IDX);
        let eventq_max_size = regs.queue_num_max.get() as usize;
        if QUEUE_SIZE > eventq_max_size {
            log_warning!("Eventq is too large. Maximum {}", eventq_max_size);
            return Err(super::Error::DeviceSpecificError);
        }

        regs.queue_num.set(QUEUE_SIZE as u32);

        let queue_desc = eventq.descriptor_table();
        regs.queue_descriptor_low.set(queue_desc.low_u32());
        regs.queue_descriptor_high.set(queue_desc.high_u32());

        let avail_ring = eventq.available_ring();
        regs.queue_driver_low.set(avail_ring.low_u32());
        regs.queue_driver_high.set(avail_ring.high_u32());

        let used_ring = eventq.used_ring();
        regs.queue_device_low.set(used_ring.low_u32());
        regs.queue_device_high.set(used_ring.high_u32());

        regs.queue_ready.set(1);

        regs.queue_sel.set(STATUSQ_IDX);
        let statusq_max_size = regs.queue_num_max.get() as usize;
        if QUEUE_SIZE > statusq_max_size {
            log_warning!("Statusq is too large. Maximum {}", statusq_max_size);
            return Err(super::Error::DeviceSpecificError);
        }

        regs.queue_num.set(QUEUE_SIZE as u32);

        let queue_desc = statusq.descriptor_table();
        regs.queue_descriptor_low.set(queue_desc.low_u32());
        regs.queue_descriptor_high.set(queue_desc.high_u32());

        let avail_ring = statusq.available_ring();
        regs.queue_driver_low.set(avail_ring.low_u32());
        regs.queue_driver_high.set(avail_ring.high_u32());

        let used_ring = statusq.used_ring();
        regs.queue_device_low.set(used_ring.low_u32());
        regs.queue_device_high.set(used_ring.high_u32());

        regs.queue_ready.set(1);

        Ok((eventq, statusq))
    }
}

macro_rules! define_enum {
    {
        $name: ident,
        $inner_type: ty,
        [
            $($field_name: ident = $field_value: literal),+
        ],
        $error_ident: ident
    } => {
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $($field_name),+
        }

        impl TryFrom<$inner_type> for $name {
            type Error = Error;
            fn try_from(value: $inner_type) -> Result<Self, Self::Error> {
                match value {
                    $( $field_value => Ok($name::$field_name),)+
                    _ => Err(Error::$error_ident(value)),
                }
            }
        }
    };
}

define_enum! {
    EventType, u16,
    [
        Sync = 0x00,
        Key = 0x01,
        Rel = 0x02,
        Abs = 0x03,
        Msc = 0x04,
        Sw = 0x05,
        Led = 0x11,
        Snd = 0x12,
        Rep = 0x14,
        Ff = 0x15,
        Pwr = 0x16,
        FfStatus = 0x17
    ],
    InvalidEventType
}

define_enum! {
    Keys, u16,
    [
        Esc = 1,
        Num1 = 2,
        Num2 = 3,
        Num3 = 4,
        Num4 = 5,
        Num5 = 6,
        Num6 = 7,
        Num7 = 8,
        Num8 = 9,
        Num9 = 10,
        Num0 = 11,
        Minus = 12,
        Equal = 13,
        Backspace = 14,
        Tab = 15,
        Q = 16,
        W = 17,
        E = 18,
        R = 19,
        T = 20,
        Y = 21,
        U = 22,
        I = 23,
        O = 24,
        P = 25,
        LeftBrace = 26,
        RightBrace = 27,
        Enter = 28,
        LeftCtrl = 29,
        A = 30,
        S = 31,
        D = 32,
        F = 33,
        G = 34,
        H = 35,
        J = 36,
        K = 37,
        L = 38,
        Semicolon = 39,
        Apostrophe = 40,
        Grave = 41,
        LeftShift = 42,
        BackSlash = 43,
        Z = 44,
        X = 45,
        C = 46,
        V = 47,
        B = 48,
        N = 49,
        M = 50,
        Comma = 51,
        Dot = 52,
        Slash = 53,
        RightShift = 54,
        KpAsterisk = 55,
        LeftAlt = 56,
        Space = 57,
        CapsLock = 58,
        F1 = 59,
        F2 = 60,
        F3 = 61,
        F4 = 62,
        F5 = 63,
        F6 = 64,
        F7 = 65,
        F8 = 66,
        F9 = 67,
        F10 = 68,
        NumLock = 69,
        ScrollLock = 70,
        Kp7 = 71,
        Kp8 = 72,
        Kp9 = 73,
        KpMinus = 74,
        Kp4 = 75,
        Kp5 = 76,
        Kp6 = 77,
        KpPlus = 78,
        Kp1 = 79,
        Kp2 = 80,
        Kp3 = 81,
        Kp0 = 82,
        KpDot = 83,

        Zenkakuhankaku = 85,
        K102nd = 86,
        F11 = 87,
        F12 = 88,
        Ro = 89,
        Katakana = 90,
        Hiragana = 91,
        Henkan = 92,
        Katakanahiragana = 93,
        Muhenkan = 94,
        Kpjpcomma = 95,
        KpEnter = 96,
        RightCtrl = 97,
        KpSlash = 98,
        Sysrq = 99,
        RightAlt = 100,
        LineFeed = 101,
        Home = 102,
        Up = 103,
        PageUp = 104,
        Left = 105,
        Right = 106,
        End = 107,
        Down = 108,
        PageDown = 109,
        Insert = 110,
        Delete = 111,
        Macro = 112,
        Mute = 113,
        VolumeDown = 114,
        VolumeUp = 115,
        Power = 116,
        /* SC System Power Down */
        KpEqual = 117,
        KpPlusMinus = 118,
        Pause = 119,
        Scale = 120,
        /* AL Compiz Scale (Expose) */
        KpComma = 121,
        Hangeul = 122,
        Hanja = 123,
        Yen = 124,
        LeftMeta = 125,
        RightMeta = 126,
        Compose = 127,

        Stop = 128,
        /* AC Stop */
        Again = 129,
        Props = 130,
        /* AC Properties */
        Undo = 131,
        /* AC Undo */
        Front = 132,
        Copy = 133,
        /* AC Copy */
        Open = 134,
        /* AC Open */
        Paste = 135,
        /* AC Paste */
        Find = 136,
        /* AC Search */
        Cut = 137,
        /* AC Cut */
        Help = 138,
        /* AL Integrated Help Center */
        Menu = 139,
        /* Menu (show menu) */
        Calc = 140,
        /* AL Calculator */
        SetUp = 141,
        Sleep = 142,
        /* SC System Sleep */
        WakeUp = 143,
        /* System Wake Up */
        File = 144,
        /* AL Local Machine Browser */
        SendFile = 145,
        DeleteFile = 146,
        Xfer = 147,
        Prog1 = 148,
        Prog2 = 149,
        Www = 150,
        /* AL Internet Browser */
        MsDos = 151,
        ScreenLock = 152,
        /* AL Terminal Lock/Screensaver */
        RotateDisplay = 153,
        /* Display orientation for e.g. tablets */
        CycleWindows = 154,
        Mail = 155,
        BookMarks = 156,
        /* AC Bookmarks */
        Computer = 157,
        Back = 158,
        /* AC Back */
        Forward = 159,
        /* AC Forward */
        CloseCd = 160,
        EjectCd = 161,
        EjectCloseCd = 162,
        NextSong = 163,
        PlayPause = 164,
        PreviousSong = 165,
        StopCd = 166,
        Record = 167,
        Rewind = 168,
        Phone = 169,
        /* Media Select Telephone */
        Iso = 170,
        Config = 171,
        /* AL Consumer Control Configuration */
        Homepage = 172,
        /* AC Home */
        Refresh = 173,
        /* AC Refresh */
        Exit = 174,
        /* AC Exit */
        Move = 175,
        Edit = 176,
        ScrollUp = 177,
        ScrollDown = 178,
        KpLeftParen = 179,
        KpRightParen = 180,
        New = 181,
        /* AC New */
        Redo = 182,
        /* AC Redo/Repeat */
        F13 = 183,
        F14 = 184,
        F15 = 185,
        F16 = 186,
        F17 = 187,
        F18 = 188,
        F19 = 189,
        F20 = 190,
        F21 = 191,
        F22 = 192,
        F23 = 193,
        F24 = 194,

        PlayCd = 200,
        PauseCd = 201,
        Prog3 = 202,
        Prog4 = 203,
        AllApplications = 204,
        /* AC Desktop Show All Applications */
        Suspend = 205,
        Close = 206,
        /* AC Close */
        Play = 207,
        FastForward = 208,
        BassBoost = 209,
        Print = 210,
        /* AC Print */
        Hp = 211,
        Camera = 212,
        Sound = 213,
        Question = 214,
        Email = 215,
        Chat = 216,
        Search = 217,
        Connect = 218,
        Finance = 219,
        /* AL Checkbook/Finance */
        Sport = 220,
        Shop = 221,
        AltErase = 222,
        Cancel = 223,
        /* AC Cancel */
        BrightnessDown = 224,
        BrightnessUp = 225,
        Media = 226,

        SwitchVideoMode = 227,
        /* Cycle between available video
        outputs (Monitor/LCD/TV-out/etc) */
        KbdIllumToggle = 228,
        KbdIllumDown = 229,
        KbdIllumUp = 230,

        Send = 231,
        /* AC Send */
        Reply = 232,
        /* AC Reply */
        ForwardMail = 233,
        /* AC Forward Msg */
        Save = 234,
        /* AC Save */
        Documents = 235,

        Battery = 236,

        Bluetooth = 237,
        Wlan = 238,
        Uwb = 239,

        Unknown = 240,

        VideoNext = 241,
        /* drive next video source */
        VideoPrev = 242,
        /* drive previous video source */
        BrightnessCycle = 243,
        /* brightness up, after max is min */
        BrightnessAuto = 244,
        /* Set Auto Brightness: manual
        brightness control is off,
        rely on ambient */
        DisplayOff = 245,
        /* display device to off state */
        Wwan = 246,
        /* Wireless WAN (LTE, UMTS, GSM, etc.) */
        RfKill = 247,
        /* Key that controls all radios */
        Micmute = 248
        /* Mute / unmute the microphone */
    ],
    InvalidKey
}

define_enum! {
    KeyState, u32,
    [
        Pressed = 1,
        Released = 0
    ],
    InvalidKeyState
}

#[derive(Debug)]
struct Event {
    _ty: EventType,
    _key: Keys,
    _state: KeyState,
}

// This is just a marker trait really
impl Subdev for InputSubdev {}
