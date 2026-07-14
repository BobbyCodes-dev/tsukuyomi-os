use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use std::time::Duration;

const LSHIFT_MAKE: u8 = 0x2A;
const ENTER_MAKE: u8 = 0x1C;

fn base_scancode(c: char) -> Option<(u8, bool)> {
    let lower = c.to_ascii_lowercase();
    let letter_code = match lower {
        'a' => 0x1E,
        'b' => 0x30,
        'c' => 0x2E,
        'd' => 0x20,
        'e' => 0x12,
        'f' => 0x21,
        'g' => 0x22,
        'h' => 0x23,
        'i' => 0x17,
        'j' => 0x24,
        'k' => 0x25,
        'l' => 0x26,
        'm' => 0x32,
        'n' => 0x31,
        'o' => 0x18,
        'p' => 0x19,
        'q' => 0x10,
        'r' => 0x13,
        's' => 0x1F,
        't' => 0x14,
        'u' => 0x16,
        'v' => 0x2F,
        'w' => 0x11,
        'x' => 0x2D,
        'y' => 0x15,
        'z' => 0x2C,
        _ => 0,
    };
    if letter_code != 0 {
        return Some((letter_code, c.is_ascii_uppercase()));
    }

    let (code, shift) = match c {
        '1' => (0x02, false),
        '!' => (0x02, true),
        '2' => (0x03, false),
        '@' => (0x03, true),
        '3' => (0x04, false),
        '#' => (0x04, true),
        '4' => (0x05, false),
        '$' => (0x05, true),
        '5' => (0x06, false),
        '%' => (0x06, true),
        '6' => (0x07, false),
        '^' => (0x07, true),
        '7' => (0x08, false),
        '&' => (0x08, true),
        '8' => (0x09, false),
        '*' => (0x09, true),
        '9' => (0x0A, false),
        '(' => (0x0A, true),
        '0' => (0x0B, false),
        ')' => (0x0B, true),
        '-' => (0x0C, false),
        '_' => (0x0C, true),
        '=' => (0x0D, false),
        '+' => (0x0D, true),
        '[' => (0x1A, false),
        '{' => (0x1A, true),
        ']' => (0x1B, false),
        '}' => (0x1B, true),
        '\\' => (0x2B, false),
        '|' => (0x2B, true),
        ';' => (0x27, false),
        ':' => (0x27, true),
        '\'' => (0x28, false),
        '"' => (0x28, true),
        '`' => (0x29, false),
        '~' => (0x29, true),
        ',' => (0x33, false),
        '<' => (0x33, true),
        '.' => (0x34, false),
        '>' => (0x34, true),
        '/' => (0x35, false),
        '?' => (0x35, true),
        ' ' => (0x39, false),
        '\n' => (ENTER_MAKE, false),
        '\t' => (0x0F, false),
        _ => return None,
    };
    Some((code, shift))
}

pub fn string_to_scancodes(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for c in s.chars() {
        let Some((code, shift)) = base_scancode(c) else { continue };
        if shift {
            out.push(LSHIFT_MAKE);
        }
        out.push(code);
        out.push(code | 0x80);
        if shift {
            out.push(LSHIFT_MAKE | 0x80);
        }
    }
    out
}

pub fn send_scancodes(vm_name: &str, codes: &[u8]) -> Result<()> {
    for chunk in codes.chunks(40) {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let mut cmd = Command::new("VBoxManage");
        cmd.arg("controlvm").arg(vm_name).arg("keyboardputscancode");
        for h in &hex {
            cmd.arg(h);
        }
        let status = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status()?;
        if !status.success() {
            bail!("VBoxManage keyboardputscancode failed for '{vm_name}'");
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    Ok(())
}

pub fn type_string(vm_name: &str, s: &str) -> Result<()> {
    send_scancodes(vm_name, &string_to_scancodes(s))
}
