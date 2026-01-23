const PADCHAR: char = '=';
const ALPHA: &str = "LVoJPiCN2R8G90yg+hmFHuacZ1OWMnrsSTXkYpUq/3dlbfKwv6xztjI7DeBE45QA";

fn getbyte(s: &[u8], i: usize) -> u32 {
    let b = s[i];
    b as u32
}

pub fn get_base64(s: &[u8]) -> String {
    let len = s.len();
    if len == 0 {
        return "".to_string();
    }

    let mut x = String::new();
    let imax = len - (len % 3);

    // 处理 3 字节块
    for i in (0..imax).step_by(3) {
        let b10 = (getbyte(s, i) << 16) | (getbyte(s, i + 1) << 8) | getbyte(s, i + 2);

        x.push(ALPHA.chars().nth(((b10 >> 18) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth(((b10 >> 12) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth(((b10 >> 6) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth((b10 & 63) as usize).unwrap());
    }

    let remain = len - imax;

    // 处理剩余 1 字节
    if remain == 1 {
        let b10 = getbyte(s, imax) << 16;
        x.push(ALPHA.chars().nth(((b10 >> 18) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth(((b10 >> 12) & 63) as usize).unwrap());
        x.push(PADCHAR);
        x.push(PADCHAR);
    }
    // 处理剩余 2 字节
    else if remain == 2 {
        let b10 = (getbyte(s, imax) << 16) | (getbyte(s, imax + 1) << 8);
        x.push(ALPHA.chars().nth(((b10 >> 18) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth(((b10 >> 12) & 63) as usize).unwrap());
        x.push(ALPHA.chars().nth(((b10 >> 6) & 63) as usize).unwrap());
        x.push(PADCHAR);
    }

    x
}
