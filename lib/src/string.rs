pub const HEX_CHARS: [char; 16] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'];

pub fn to_hex_str<T>(bytes: T) -> String
where
    T: AsRef<[u8]>
{
    let slice = bytes.as_ref();
    let mut rtn = String::with_capacity(slice.len() * 2);

    for byte in slice {
        let upper = ((byte & 0xF0) >> 4) as usize;
        let lower = (byte & 0x0F) as usize;

        rtn.push(HEX_CHARS[upper]);
        rtn.push(HEX_CHARS[lower]);
    }

    rtn
}

pub fn from_hex_char(ch: char) -> Option<u8> {
    match ch {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'a' | 'A' => Some(10),
        'b' | 'B' => Some(11),
        'c' | 'C' => Some(12),
        'd' | 'D' => Some(13),
        'e' | 'E' => Some(14),
        'f' | 'F' => Some(15),
        _ => None
    }
}

pub fn from_hex_str(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }

    let mut rtn = Vec::with_capacity(hex.len() / 2);
    let mut iter = hex.chars();

    while let Some(upper) = iter.next() {
        let Some(lower) = iter.next() else {
            return None;
        };

        let Some(upper_nibble) = from_hex_char(upper) else {
            return None;
        };
        let Some(lower_nibble) = from_hex_char(lower) else {
            return None;
        };

        rtn.push((upper_nibble << 4) | lower_nibble);
    }

    Some(rtn)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn to_hex() {
        let bytes = vec![0x01, 0x09, 0x0a, 0x0f, 0x10, 0x90, 0xa0, 0xf0, 0x11, 0x99, 0xaa, 0xff];
        let expected = String::from("01090A0F1090A0F01199AAFF");

        let result = to_hex_str(&bytes);

        assert_eq!(expected, result);
    }

    #[test]
    fn from_hex() {
        let hex = String::from("01090A0F1090A0F01199AAFF");
        let expected = Some(vec![0x01, 0x09, 0x0a, 0x0f, 0x10, 0x90, 0xa0, 0xf0, 0x11, 0x99, 0xaa, 0xff]);

        let result = from_hex_str(&hex);

        assert_eq!(expected, result);
    }
}
