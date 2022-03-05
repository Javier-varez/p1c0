// TODO(jalv): Add missing entries here
static SCAN_TABLE: [Option<char>; 256] = [
    None,
    None,
    None,
    None,
    Some('A'),
    Some('B'),
    Some('C'),
    Some('D'),
    Some('E'),
    Some('F'),
    Some('G'),
    Some('H'),
    Some('I'),
    Some('J'),
    Some('K'),
    Some('L'),
    Some('M'),
    Some('N'),
    Some('O'),
    Some('P'),
    Some('Q'),
    Some('R'),
    Some('S'),
    Some('T'),
    Some('U'),
    Some('V'),
    Some('W'),
    Some('X'),
    Some('Y'),
    Some('Z'),
    Some('1'),
    Some('2'),
    Some('3'),
    Some('4'),
    Some('5'),
    Some('6'),
    Some('7'),
    Some('8'),
    Some('9'),
    Some('0'),
    Some('\n'),
    None,
    None,
    Some('\t'),
    Some(' '),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Scancode(u8);

impl Scancode {
    pub const fn new(value: u8) -> Self {
        Scancode(value)
    }

    pub fn to_char(&self) -> Option<char> {
        SCAN_TABLE[self.0 as usize]
    }

    pub fn is_error(&self) -> bool {
        self.0 == 1
    }

    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug)]
pub struct KeyboardReport {
    _modifiers: u8,
    keycodes: [Scancode; 6],
}

impl KeyboardReport {
    pub fn new(data: &[u8]) -> Self {
        Self {
            _modifiers: data[1],
            keycodes: [
                Scancode::new(data[3]),
                Scancode::new(data[4]),
                Scancode::new(data[5]),
                Scancode::new(data[6]),
                Scancode::new(data[7]),
                Scancode::new(data[8]),
            ],
        }
    }
    pub fn keycodes(&self) -> &[Scancode] {
        &self.keycodes
    }

    pub fn has_error(&self) -> bool {
        self.keycodes.iter().any(|code| code.is_error())
    }
}

pub struct Keyboard {
    current_keycodes: [Scancode; 6],
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            current_keycodes: [Scancode::new(0); 6],
        }
    }

    fn key_pressed(&mut self, code: Scancode) {
        // Insert in current_keycodes
        for keycode in &mut self.current_keycodes {
            if !keycode.is_valid() {
                *keycode = code;
                break;
            }
        }

        // TODO(javier-varez): Send key-down event
        if let Some(c) = code.to_char() {
            crate::print!("{}", c);
        }
    }

    pub fn handle_report(&mut self, report: KeyboardReport) {
        // Ignore error reports
        if report.has_error() {
            crate::println!("Too many keys pressed");
            return;
        }

        // TODO(javier-varez): Handle modifiers

        // Remove keys that are not pressed anymore
        for keycode in self
            .current_keycodes
            .iter_mut()
            .filter(|keycode| keycode.is_valid())
        {
            if !report.keycodes().iter().any(|code| *code == *keycode) {
                *keycode = Scancode::new(0);
                // TODO(javier-varez): Send key-up event
            }
        }

        // Check for key-down events
        for keycode in report
            .keycodes()
            .iter()
            .filter(|keycode| keycode.is_valid())
        {
            if !self.current_keycodes.iter().any(|code| *code == *keycode) {
                // Insert keycode
                self.key_pressed(*keycode);
            }
        }
    }
}
