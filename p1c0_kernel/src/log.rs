#[macro_export]
macro_rules! ansi_escape_reset {
    () => {
        "\x1b[0;0m"
    };
}

#[macro_export]
macro_rules! ansi_escape_gray {
    () => {
        "\x1b[2;37m"
    };
}

#[macro_export]
macro_rules! ansi_escape_red {
    () => {
        "\x1b[1;31m"
    };
}

#[macro_export]
macro_rules! ansi_escape_magenta {
    () => {
        "\x1b[1;35m"
    };
}

#[macro_export]
macro_rules! ansi_escape_green {
    () => {
        "\x1b[1;32m"
    };
}

#[macro_export]
macro_rules! ansi_escape_blue {
    () => {
        "\x1b[1;34m"
    };
}

#[macro_export]
macro_rules! ansi_escape_dimmed_blue {
    () => {
        "\x1b[2;34m"
    };
}

#[macro_export]
macro_rules! _log {
    ($level: expr, $level_str: expr, $format: literal $(, $($args: tt)+)?) => {
        $crate::log::_print_log(
            $level,
            ::core::format_args!(
                ::core::concat!($level_str,
                                "{}: ",
                                $crate::ansi_escape_reset!(),
                                $format,
                                $crate::ansi_escape_gray!(),
                                "\n└── File: {}, Line: {}\n",
                                $crate::ansi_escape_reset!()),
                ::core::module_path!(),
                $($($args)+ ,)?
                ::core::file!(),
                ::core::line!()
            ),
        );
    };
}

#[macro_export]
macro_rules! log_error {
    ($format: literal $(, $($args: tt)*)?) => {
        $crate::_log!(
            $crate::log::Level::Error,
            $crate::ansi_escape_red!(),
            $format $(, $($args)*)?);
    };
}

#[macro_export]
macro_rules! log_warning {
    ($format: literal $(, $($args: tt)+)?) => {
        $crate::_log!(
            $crate::log::Level::Warning,
            $crate::ansi_escape_magenta!(),
            $format $(, $($args)+)?);
    };
}

#[macro_export]
macro_rules! log_info {
    ($format: literal $(, $($args: tt)+)?) => {
        $crate::_log!(
            $crate::log::Level::Info,
            $crate::ansi_escape_blue!(),
            $format $(, $($args)+)?);
    };
}

#[macro_export]
macro_rules! log_debug {
    ($format: literal $(, $($args: tt)+)?) => {
        $crate::_log!(
            $crate::log::Level::Debug,
            $crate::ansi_escape_green!(),
            $format $(, $($args)+)?);
    };
}

#[macro_export]
macro_rules! log_verbose {
    ($format: literal $(, $($args: tt)+)?) => {
        $crate::_log!(
            $crate::log::Level::Debug,
            $crate::ansi_escape_dimmed_blue!(),
            $format $(, $($args)+)?);
    };
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone)]
pub enum Level {
    None = 0,
    Error = 1,
    Warning = 2,
    Info = 3,
    Debug = 4,
    Verbose = 5,
}

impl From<u8> for Level {
    fn from(val: u8) -> Self {
        match val {
            0 => Level::None,
            1 => Level::Error,
            2 => Level::Warning,
            3 => Level::Info,
            4 => Level::Debug,
            5 => Level::Verbose,
            val => {
                panic!("Unknown log level {}", val);
            }
        }
    }
}

/// TODO(javier-varez): Make this configurable in runtime and also build time
/// Let's start off with Debug for now given that we are still in development
static LEVEL: u8 = Level::Debug as u8;

#[doc(hidden)]
pub fn _print_log(level: Level, format_args: core::fmt::Arguments) {
    let current_level = LEVEL.into();
    if level <= current_level {
        crate::_print(format_args);
    }
}
