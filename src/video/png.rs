//! PNG の書き出し (外部ツールなし)。
//!
//! 確認用の 1 コマを保存するだけなので、圧縮はしない (deflate の「無圧縮ブロック」を使う)。
//! 1280×720 で約 2.8 MB になるが、どのビューアでも開ける正しい PNG になる。

/// CRC-32 (PNG のチャンクの検査値)
fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for data in chunks {
        for &b in *data {
            crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
        }
    }
    crc ^ 0xFFFF_FFFF
}

/// Adler-32 (zlib の検査値)
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

/// チャンクを 1 つ書く。長さが 32 ビットに収まらなければ None
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) -> Option<()> {
    out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
    Some(())
}

/// RGB (1 画素 3 バイト、左上から横に並んだもの) を PNG のバイト列にする。
/// 大きさが合わないときは None。
pub fn encode_rgb(width: usize, height: usize, rgb: &[u8]) -> Option<Vec<u8>> {
    let row = width.checked_mul(3)?;
    if width == 0 || height == 0 || rgb.len() != row.checked_mul(height)? {
        return None;
    }
    let (w32, h32) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);

    // 各行の先頭にフィルタ番号 0 (なし) を付けたもの
    let mut raw = Vec::with_capacity((row + 1) * height);
    for line in rgb.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    // zlib: ヘッダ + 無圧縮ブロック (最大 65535 バイトずつ) + Adler-32
    let mut z = Vec::with_capacity(raw.len() + raw.len() / 65_535 * 5 + 16);
    z.extend_from_slice(&[0x78, 0x01]);
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(b) = blocks.next() {
        let last = blocks.peek().is_none();
        z.push(u8::from(last));
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = Vec::with_capacity(z.len() + 64);
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w32.to_be_bytes());
    ihdr.extend_from_slice(&h32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit、RGB、deflate、フィルタ方式 0、インターレースなし
    chunk(&mut out, b"IHDR", &ihdr)?;
    chunk(&mut out, b"IDAT", &z)?;
    chunk(&mut out, b"IEND", &[])?;
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn png_layout() {
        let png = encode_rgb(2, 1, &[255, 0, 0, 0, 0, 255]).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[png.len() - 12..], &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]);
        // 65535 バイトを超える画像でもブロックに分かれる
        let big = vec![7u8; 300 * 100 * 3];
        assert!(encode_rgb(300, 100, &big).is_some());
    }

    #[test]
    fn wrong_size_is_rejected() {
        assert!(encode_rgb(2, 2, &[0; 11]).is_none());
        assert!(encode_rgb(0, 2, &[]).is_none());
    }
}
