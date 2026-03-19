const PADCHAR: u8 = b'=';
const ALPHA: &[u8; 64] = b"LVoJPiCN2R8G90yg+hmFHuacZ1OWMnrsSTXkYpUq/3dlbfKwv6xztjI7DeBE45QA";

pub fn get_base64(s: &[u8]) -> String {
    let len = s.len();
    if len == 0 {
        return String::new();
    }

    let mut out = Vec::with_capacity((len + 2) / 3 * 4);
    let imax = len - (len % 3);

    for i in (0..imax).step_by(3) {
        let b10 = (s[i] as u32) << 16 | (s[i + 1] as u32) << 8 | s[i + 2] as u32;
        out.push(ALPHA[((b10 >> 18) & 63) as usize]);
        out.push(ALPHA[((b10 >> 12) & 63) as usize]);
        out.push(ALPHA[((b10 >> 6) & 63) as usize]);
        out.push(ALPHA[(b10 & 63) as usize]);
    }

    let remain = len - imax;
    if remain == 1 {
        let b10 = (s[imax] as u32) << 16;
        out.push(ALPHA[((b10 >> 18) & 63) as usize]);
        out.push(ALPHA[((b10 >> 12) & 63) as usize]);
        out.push(PADCHAR);
        out.push(PADCHAR);
    } else if remain == 2 {
        let b10 = (s[imax] as u32) << 16 | (s[imax + 1] as u32) << 8;
        out.push(ALPHA[((b10 >> 18) & 63) as usize]);
        out.push(ALPHA[((b10 >> 12) & 63) as usize]);
        out.push(ALPHA[((b10 >> 6) & 63) as usize]);
        out.push(PADCHAR);
    }

    // SAFETY: ALPHA only contains ASCII bytes, PADCHAR is ASCII
    unsafe { String::from_utf8_unchecked(out) }
}
