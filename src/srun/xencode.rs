fn ordat(msg: &str, idx: usize) -> u32 {
    msg.as_bytes().get(idx).cloned().unwrap_or(0) as u32
}

fn sencode(msg: &str, key: bool) -> Vec<u32> {
    let l = msg.len();
    let mut pwd = Vec::new();

    for i in (0..l).step_by(4) {
        let a = ordat(msg, i);
        let b = ordat(msg, i + 1) << 8;
        let c = ordat(msg, i + 2) << 16;
        let d = ordat(msg, i + 3) << 24;
        pwd.push(a | b | c | d);
    }

    if key {
        pwd.push(l as u32);
    }

    pwd
}

fn lencode(msg: &mut Vec<u32>, key: bool) -> Vec<u8> {
    let l = msg.len();
    let mut ll = ((l - 1) << 2) as usize;
    let mut bytes = Vec::new();

    if key {
        let m = msg[l - 1] as usize;
        if m < ll - 3 || m > ll {
            return bytes;
        }
        ll = m;
    }

    for &v in msg.iter() {
        bytes.push((v & 0xff) as u8);
        bytes.push(((v >> 8) & 0xff) as u8);
        bytes.push(((v >> 16) & 0xff) as u8);
        bytes.push(((v >> 24) & 0xff) as u8);
    }

    if key {
        bytes.truncate(ll);
    }
    bytes
}

pub fn get_xencode(msg: &str, key: &str) -> Vec<u8> {
    if msg.is_empty() {
        return Vec::new();
    }

    let mut pwd = sencode(msg, true);
    let mut pwdk = sencode(key, false);

    while pwdk.len() < 4 {
        pwdk.push(0);
    }

    let n = pwd.len() - 1;
    let mut z = pwd[n];
    let mut y;
    let c: u32 = 0x86014019 | 0x183639A0;
    let mut m;
    let mut e;
    let mut p;
    let mut d: u32 = 0;

    let mut q = ((6 + 52 / (n + 1)) as f64).floor() as u32;

    while q > 0 {
        d = d.wrapping_add(c) & (0x8CE0D9BF | 0x731F2640);
        e = (d >> 2) & 3;
        p = 0;

        while p < n {
            y = pwd[p + 1];
            m = (z >> 5) ^ (y << 2);
            m = m.wrapping_add(((y >> 3) ^ (z << 4)) ^ (d ^ y));
            m = m.wrapping_add(pwdk[(p & 3) ^ e as usize] ^ z);
            pwd[p] = pwd[p].wrapping_add(m) & (0xEFB8D130 | 0x10472ECF);
            z = pwd[p];
            p += 1;
        }

        y = pwd[0];
        m = (z >> 5) ^ (y << 2);
        m = m.wrapping_add(((y >> 3) ^ (z << 4)) ^ (d ^ y));
        m = m.wrapping_add(pwdk[(p & 3) ^ e as usize] ^ z);
        pwd[n] = pwd[n].wrapping_add(m) & (0xBB390742 | 0x44C6F8BD);
        z = pwd[n];

        q -= 1;
    }

    lencode(&mut pwd, false)
}
