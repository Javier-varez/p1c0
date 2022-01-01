#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_arch = "arm", feature(isa_attribute))]

#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), path = "arch/x86.rs")]
pub mod arch;

pub mod io;

use io::{CloseArgs, FlenArgs, OpenArgs, ReadArgs, RemoveArgs, RenameArgs, SeekArgs, WriteArgs};

use core::convert::From;
use core::fmt::Display;

#[derive(Debug)]
pub enum Error {
    CouldNotReadExtensions,
    IoError(io::Error),
}

impl From<io::Error> for Error {
    fn from(io_err: io::Error) -> Self {
        Error::IoError(io_err)
    }
}

#[derive(Debug)]
pub struct Errno(pub u32);

impl Errno {
    fn get_code_description(&self) -> Option<&str> {
        match self.0 {
            1 => Some("EPERM: Operation not permitted"),
            2 => Some("ENOENT: No such file or directory"),
            3 => Some("ESRCH: No such process"),
            4 => Some("EINTR: Interrupted system call"),
            5 => Some("EIO: I/O error"),
            6 => Some("ENXIO: No such device or address"),
            7 => Some("E2BIG: Argument list too long"),
            8 => Some("ENOEXEC: Exec format error"),
            9 => Some("EBADF: Bad file number"),
            10 => Some("ECHILD: No child processes"),
            11 => Some("EAGAIN: Try again"),
            12 => Some("ENOMEM: Out of memory"),
            13 => Some("EACCES: Permission denied"),
            14 => Some("EFAULT: Bad address"),
            15 => Some("ENOTBLK: Block device required"),
            16 => Some("EBUSY: Device or resource busy"),
            17 => Some("EEXIST: File exists"),
            18 => Some("EXDEV: Cross-device link"),
            19 => Some("ENODEV: No such device"),
            20 => Some("ENOTDIR: Not a directory"),
            21 => Some("EISDIR: Is a directory"),
            22 => Some("EINVAL: Invalid argument"),
            23 => Some("ENFILE: File table overflow"),
            24 => Some("EMFILE: Too many open files"),
            25 => Some("ENOTTY: Not a typewriter"),
            26 => Some("ETXTBSY: Text file busy"),
            27 => Some("EFBIG: File too large"),
            28 => Some("ENOSPC: No space left on device"),
            29 => Some("ESPIPE: Illegal seek"),
            30 => Some("EROFS: Read-only file system"),
            31 => Some("EMLINK: Too many links"),
            32 => Some("EPIPE: Broken pipe"),
            33 => Some("EDOM: Math argument out of domain of func"),
            34 => Some("ERANGE: Math result not representable"),
            38 => Some("ENOSYS: Invalid system call number"),
            39 => Some("ENOTEMPTY: Directory not empty"),
            40 => Some("ELOOP: Too many symbolic links encountered"),
            42 => Some("ENOMSG: No message of desired type"),
            43 => Some("EIDRM: Identifier removed"),
            44 => Some("ECHRNG: Channel number out of range"),
            45 => Some("EL2NSYNC: Level 2 not synchronized"),
            46 => Some("EL3HLT: Level 3 halted"),
            47 => Some("EL3RST: Level 3 reset"),
            48 => Some("ELNRNG: Link number out of range"),
            49 => Some("EUNATCH: Protocol driver not attached"),
            50 => Some("ENOCSI: No CSI structure available"),
            51 => Some("EL2HLT: Level 2 halted"),
            52 => Some("EBADE: Invalid exchange"),
            53 => Some("EBADR: Invalid rquest descriptor"),
            54 => Some("EXFULL: Exchange full"),
            55 => Some("ENOANO: No anode"),
            56 => Some("EBADRQC: Invalid request code"),
            57 => Some("EBADSLT: Invalid slot"),
            59 => Some("EBFONT: Bad font file format"),
            60 => Some("ENOSTR: Device not a stream"),
            61 => Some("ENODATA: No data available"),
            62 => Some("ETIME: Timer expired"),
            63 => Some("ENOSR: Out of streams resources"),
            64 => Some("ENONET: Machine is not on the network"),
            65 => Some("ENOPKG: Package not installed"),
            66 => Some("EREMOTE: Object is remote"),
            67 => Some("ENOLINK: Link has been severed"),
            68 => Some("EADV: Advertise error"),
            69 => Some("ESRMNT: Srmount error"),
            70 => Some("ECOMM: Communication error on send"),
            71 => Some("EPROTO: Protocol error"),
            72 => Some("EMULTIHOP: Multihop attempted"),
            73 => Some("EDOTDOT: RFS specific error"),
            74 => Some("EBADMSG: Not a data message"),
            75 => Some("EOVERFLOW: Value too large for defined data type"),
            76 => Some("ENOTUNIQ: Name not unique on network"),
            77 => Some("EBADFD: File descriptor in bad state"),
            78 => Some("EREMCHG: Remote address changed"),
            79 => Some("ELIBACC: Can not access a needed shared library"),
            80 => Some("ELIBBAD: Accessing a corrupted shared library"),
            81 => Some("ELIBSCN: .lib section in a.out corrupted"),
            82 => Some("ELIBMAX: Attempting to link in too many shared libraries"),
            83 => Some("ELIBEXEC: Cannot exec a shared library directly"),
            84 => Some("EILSEQ: Illegal byte sequence"),
            85 => Some("ERESTART: Interrupted system call should be restarted"),
            86 => Some("ESTRPIPE: Streams pipe error"),
            87 => Some("EUSERS: Too many users"),
            88 => Some("ENOTSOCK: Socket operation on non-socket"),
            89 => Some("EDESTADDRREQ: Destination address required"),
            90 => Some("EMSGSIZE: Message too long"),
            91 => Some("EPROTOTYPE: Protocol wrong type for socket"),
            92 => Some("ENOPROTOOPT: Protocol not available"),
            93 => Some("EPROTONOSUPPORT: Protocol not supported"),
            94 => Some("ESOCKTNOSUPPORT: Socket type not supported"),
            95 => Some("EOPNOTSUPP: Operation not supported on transport endpoint"),
            96 => Some("EPFNOSUPPORT: Protocol family not supported"),
            97 => Some("EAFNOSUPPORT: Address family not supported by protocol"),
            98 => Some("EADDRINUSE: Address already in use"),
            99 => Some("EADDRNOTAVAIL: Cannot assign requested address"),
            100 => Some("ENETDOWN: Network is down"),
            101 => Some("ENETUNREACH: Network is unreachable"),
            102 => Some("ENETRESET: Network dropped connection because of reset"),
            103 => Some("ECONNABORTED: Software caused connection abort"),
            104 => Some("ECONNRESET: Connection reset by peer"),
            105 => Some("ENOBUFS: No buffer space available"),
            106 => Some("EISCONN: Transport endpoint is already connected"),
            107 => Some("ENOTCONN: Transport endpoint is not connected"),
            108 => Some("ESHUTDOWN: Cannot send after transport endpoint shutdown"),
            109 => Some("ETOOMANYREFS: Too many references: cannot splice"),
            110 => Some("ETIMEDOUT: Connection timed out"),
            111 => Some("ECONNREFUSED: Connection refused"),
            112 => Some("EHOSTDOWN: Host is down"),
            113 => Some("EHOSTUNREACH: No route to host"),
            114 => Some("EALREADY: Operation already in progress"),
            115 => Some("EINPROGRESS: Operation now in progress"),
            116 => Some("ESTALE: Stale file handle"),
            117 => Some("EUCLEAN: Structure needs cleaning"),
            118 => Some("ENOTNAM: Not a XENIX named type file"),
            119 => Some("ENAVAIL: No XENIX semaphores available"),
            120 => Some("EISNAM: Is a named type file"),
            121 => Some("EREMOTEIO: Remote I/O error"),
            122 => Some("EDQUOT: Quota exceeded"),
            123 => Some("ENOMEDIUM: No medium found"),
            124 => Some("EMEDIUMTYPE: Wrong medium type"),
            125 => Some("ECANCELED: Operation Canceled"),
            126 => Some("ENOKEY: Required key not available"),
            127 => Some("EKEYEXPIRED: Key has expired"),
            128 => Some("EKEYREVOKED: Key has been revoked"),
            129 => Some("EKEYREJECTED: Key was rejected by service"),
            130 => Some("EOWNERDEAD: Owner died"),
            131 => Some("ENOTRECOVERABLE: State not recoverable"),
            132 => Some("ERFKILL: Operation not possible due to RF-kill"),
            133 => Some("EHWPOISON: Memory page has hardware error"),
            _ => None,
        }
    }
}

impl Display for Errno {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get_code_description() {
            Some(desc) => f.write_fmt(format_args!("Errno({}) => `{}`", self.0, desc)),
            None => f.write_fmt(format_args!("Errno({})", self.0)),
        }
    }
}

#[repr(usize)]
enum ExitReason {
    ApplicationExit = 0x20026,
}

#[repr(usize)]
enum Operation<'a> {
    Open(OpenArgs),
    Close(CloseArgs),
    Read(ReadArgs<'a>),
    Write(WriteArgs<'a>),
    Seek(SeekArgs),
    Flen(FlenArgs),
    Remove(RemoveArgs),
    Rename(RenameArgs),
    ExitExtended(ExitArgs),
    Iserror(IserrorArgs),
    Errno,
}

trait PointerArgs {
    #[inline]
    fn get_args(&self) -> usize {
        self as *const _ as *const () as usize
    }
}

#[repr(C)]
struct ExitArgs {
    sh_reason: ExitReason,
    exit_code: usize,
}

impl PointerArgs for ExitArgs {}

#[repr(C)]
struct IserrorArgs {
    code: isize,
}

impl PointerArgs for IserrorArgs {}

impl<'a> Operation<'a> {
    #[inline]
    fn code(&self) -> usize {
        match *self {
            Operation::Open(_) => 0x01,
            Operation::Close(_) => 0x02,
            Operation::Write(_) => 0x05,
            Operation::Read(_) => 0x06,
            Operation::Seek(_) => 0x0A,
            Operation::Flen(_) => 0x0C,
            Operation::Remove(_) => 0x0E,
            Operation::Rename(_) => 0x0F,
            Operation::Iserror(_) => 0x08,
            Operation::Errno => 0x13,
            Operation::ExitExtended(_) => 0x20,
        }
    }

    #[inline]
    fn args(&self) -> usize {
        match self {
            Operation::Open(args) => args.get_args(),
            Operation::Close(args) => args.get_args(),
            Operation::Write(args) => args.get_args(),
            Operation::Read(args) => args.get_args(),
            Operation::Seek(args) => args.get_args(),
            Operation::Flen(args) => args.get_args(),
            Operation::Remove(args) => args.get_args(),
            Operation::Rename(args) => args.get_args(),
            Operation::Iserror(args) => args.get_args(),
            Operation::Errno => 0,
            Operation::ExitExtended(args) => args.get_args(),
        }
    }
}

fn get_error(code: isize) -> Option<Errno> {
    let op = Operation::Iserror(IserrorArgs { code });
    let result = unsafe { arch::call_host_unchecked(&op) };

    if result == 0 {
        None
    } else {
        let op = Operation::Errno;
        Some(Errno(unsafe { arch::call_host_unchecked(&op) as u32 }))
    }
}

fn call_host(op: &Operation) -> Result<usize, Errno> {
    let result = unsafe { arch::call_host_unchecked(op) };

    if let Some(err) = get_error(result) {
        Err(err)
    } else {
        Ok(result as usize)
    }
}

pub fn exit(exit_code: u32) -> ! {
    let op = Operation::ExitExtended(ExitArgs {
        sh_reason: ExitReason::ApplicationExit,
        exit_code: exit_code as usize,
    });

    call_host(&op).ok();
    unreachable!();
}

#[derive(Debug, Default)]
pub struct Extensions {
    extended_exit: bool,
    stdout_stderr: bool,
}

impl Extensions {
    pub fn supports_extended_exit(&self) -> bool {
        self.extended_exit
    }
    pub fn supports_stdout_stderr(&self) -> bool {
        self.stdout_stderr
    }
}

pub fn load_extensions() -> Result<Extensions, Error> {
    let mut extensions_file = io::open(":semihosting-features", io::AccessType::Binary)?;

    let mut buffer = [0u8; 5];
    let total_read = extensions_file.read(&mut buffer)?;

    const EXPECTED_MAGIC: [u8; 4] = [0x53, 0x48, 0x46, 0x42];

    match total_read.cmp(&4) {
        core::cmp::Ordering::Less => Err(Error::CouldNotReadExtensions),
        core::cmp::Ordering::Equal => Ok(Extensions::default()),
        core::cmp::Ordering::Greater if buffer[..4] != EXPECTED_MAGIC => {
            Err(Error::CouldNotReadExtensions)
        }
        core::cmp::Ordering::Greater => Ok(Extensions {
            extended_exit: (buffer[4] & (1 << 0)) != 0,
            stdout_stderr: (buffer[4] & (1 << 1)) != 0,
        }),
    }
}
