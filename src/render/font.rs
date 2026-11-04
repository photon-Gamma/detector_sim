//! 最小限の TrueType (glyf アウトライン) フォント読み込み。
//!
//! 対応: .ttf / .ttc (先頭のフォント), cmap format 4 / 12, 単純グリフ + 複合グリフ。
//! CFF アウトラインの .otf (Noto CJK など) は非対応。
//!
//! フォントファイルは外部入力なので、読み出しはすべて範囲チェック付き。
//! 壊れたファイルでもパニックせず、エラーか「その文字は無い」として扱う。

use crate::error::{AppError, Result};
use crate::render::raster::Pt;
use std::collections::HashMap;
use std::path::Path;

/// グリフの輪郭 (フォント単位, y 上向き) と送り幅
pub struct Outline {
    pub contours: Vec<Vec<Pt>>,
    pub advance: f32,
}

pub struct Font {
    data: Vec<u8>,
    upem: f32,
    long_loca: bool,
    loca: usize,
    glyf: usize,
    hmtx: usize,
    n_hmetrics: u16,
    cmap4: Option<usize>,
    cmap12: Option<usize>,
}

// ------------------------------------------------ 範囲チェック付きの読み出し
fn u8_at(d: &[u8], o: usize) -> Option<u8> {
    d.get(o).copied()
}
fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o.checked_add(1)?)?]))
}
fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(o)?, *d.get(o + 1)?, *d.get(o + 2)?, *d.get(o + 3)?]))
}

/// 単純グリフの点の上限 (異常なファイルでメモリを使い過ぎないため)
const MAX_POINTS: usize = 20_000;
/// 複合グリフの入れ子の上限
const MAX_DEPTH: u32 = 6;

impl Font {
    pub fn load(path: &Path) -> Result<Font> {
        let data = std::fs::read(path).map_err(AppError::io_path("フォントを読めません", path))?;
        Font::parse(data).map_err(|m| AppError::Font(format!("{}: {}", path.display(), m)))
    }

    pub fn parse(data: Vec<u8>) -> std::result::Result<Font, String> {
        let bad = || "フォントファイルの形式が正しくありません".to_string();
        let d = &data;
        let base = if d.get(0..4) == Some(b"ttcf") { u32_at(d, 12).ok_or_else(bad)? as usize } else { 0 };
        let n = u16_at(d, base + 4).ok_or_else(bad)? as usize;
        let mut tables: HashMap<[u8; 4], usize> = HashMap::new();
        for i in 0..n {
            let r = base + 12 + 16 * i;
            let tag = d.get(r..r + 4).ok_or_else(bad)?;
            let off = u32_at(d, r + 8).ok_or_else(bad)? as usize;
            if off < d.len() {
                tables.insert([tag[0], tag[1], tag[2], tag[3]], off);
            }
        }
        let get = |k: &[u8; 4]| tables.get(k).copied().ok_or(format!("{} テーブルがありません", String::from_utf8_lossy(k)));
        let glyf = get(b"glyf").map_err(|_| "glyf テーブルがありません (CFF 形式の .otf は非対応。TrueType フォントを指定してください)".to_string())?;
        let head = get(b"head")?;
        let hhea = get(b"hhea")?;
        let cmap = get(b"cmap")?;
        let loca = get(b"loca")?;
        let hmtx = get(b"hmtx")?;

        let (mut cmap4, mut cmap12) = (None, None);
        for i in 0..u16_at(d, cmap + 2).ok_or_else(bad)? as usize {
            let r = cmap + 4 + 8 * i;
            let (Some(pid), Some(eid), Some(off)) = (u16_at(d, r), u16_at(d, r + 2), u32_at(d, r + 4)) else { break };
            let off = cmap + off as usize;
            match (u16_at(d, off), pid, eid) {
                (Some(4), 3, 1) | (Some(4), 0, _) => cmap4 = Some(off),
                (Some(12), 3, 10) | (Some(12), 0, _) => cmap12 = Some(off),
                _ => {}
            }
        }
        if cmap4.is_none() && cmap12.is_none() {
            return Err("Unicode の文字表 (cmap) がありません".into());
        }
        let upem = u16_at(d, head + 18).ok_or_else(bad)?;
        if upem == 0 {
            return Err(bad());
        }
        let n_hmetrics = u16_at(d, hhea + 34).ok_or_else(bad)?.max(1);
        Ok(Font {
            upem: f32::from(upem),
            long_loca: i16_at(d, head + 50).ok_or_else(bad)? == 1,
            loca,
            glyf,
            hmtx,
            n_hmetrics,
            cmap4,
            cmap12,
            data,
        })
    }

    pub fn units_per_em(&self) -> f32 {
        self.upem
    }

    /// 文字 → グリフ番号 (無ければ 0)
    pub fn glyph_id(&self, ch: char) -> u16 {
        let c = ch as u32;
        if let Some(g) = self.cmap12.and_then(|o| self.lookup12(o, c)) {
            return g;
        }
        if c <= 0xFFFF {
            if let Some(g) = self.cmap4.and_then(|o| self.lookup4(o, c)) {
                return g;
            }
        }
        0
    }

    fn lookup12(&self, o: usize, c: u32) -> Option<u16> {
        let d = &self.data;
        let n = u32_at(d, o + 12)? as usize;
        for i in 0..n {
            let g = o + 16 + 12 * i;
            let (s, e, sg) = (u32_at(d, g)?, u32_at(d, g + 4)?, u32_at(d, g + 8)?);
            if (s..=e).contains(&c) {
                return u16::try_from(sg.checked_add(c - s)?).ok();
            }
        }
        None
    }

    fn lookup4(&self, o: usize, c: u32) -> Option<u16> {
        let d = &self.data;
        let segx2 = u16_at(d, o + 6)? as usize;
        let ends = o + 14;
        let starts = ends + segx2 + 2;
        let deltas = starts + segx2;
        let ranges = deltas + segx2;
        for i in 0..segx2 / 2 {
            let end = u32::from(u16_at(d, ends + 2 * i)?);
            if c > end {
                continue;
            }
            let start = u32::from(u16_at(d, starts + 2 * i)?);
            if c < start {
                return None;
            }
            let delta = u16_at(d, deltas + 2 * i)?;
            let ro = u16_at(d, ranges + 2 * i)? as usize;
            if ro == 0 {
                return Some((c as u16).wrapping_add(delta));
            }
            let gi = u16_at(d, ranges + 2 * i + ro + 2 * (c - start) as usize)?;
            return if gi == 0 { None } else { Some(gi.wrapping_add(delta)) };
        }
        None
    }

    fn advance(&self, g: u16) -> f32 {
        let i = g.min(self.n_hmetrics - 1) as usize;
        u16_at(&self.data, self.hmtx + 4 * i).map(f32::from).unwrap_or(self.upem / 2.0)
    }

    fn glyph_range(&self, g: u16) -> Option<(usize, usize)> {
        let d = &self.data;
        let g = g as usize;
        let (s, e) = if self.long_loca {
            (u32_at(d, self.loca + 4 * g)? as usize, u32_at(d, self.loca + 4 * g + 4)? as usize)
        } else {
            (u16_at(d, self.loca + 2 * g)? as usize * 2, u16_at(d, self.loca + 2 * g + 2)? as usize * 2)
        };
        (s <= e && self.glyf + e <= d.len()).then_some((s, e))
    }

    /// 文字の輪郭。フォントに無い文字・壊れたグリフは None。
    pub fn outline(&self, ch: char) -> Option<Outline> {
        let g = self.glyph_id(ch);
        if g == 0 && ch != ' ' {
            return None;
        }
        let mut contours = Vec::new();
        self.glyph(g, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], &mut contours, 0)?;
        Some(Outline { contours, advance: self.advance(g) })
    }

    /// m = [a, b, c, d, dx, dy]  (x' = a·x + c·y + dx,  y' = b·x + d·y + dy)
    fn glyph(&self, g: u16, m: [f32; 6], out: &mut Vec<Vec<Pt>>, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let (s, e) = self.glyph_range(g)?;
        if e == s {
            return Some(()); // 空白など輪郭なし
        }
        let o = self.glyf + s;
        let nc = i16_at(&self.data, o)?;
        let tf = |x: f32, y: f32| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        if nc >= 0 {
            self.simple_glyph(o, nc as usize, &tf, out)
        } else {
            self.composite_glyph(o, m, &tf, out, depth)
        }
    }

    fn simple_glyph(&self, o: usize, nc: usize, tf: &dyn Fn(f32, f32) -> Pt, out: &mut Vec<Vec<Pt>>) -> Option<()> {
        let d = &self.data;
        let ends: Vec<usize> = (0..nc).map(|i| u16_at(d, o + 10 + 2 * i).map(usize::from)).collect::<Option<_>>()?;
        let np = ends.last().map_or(0, |e| e + 1);
        if np > MAX_POINTS {
            return None;
        }
        let ilen = u16_at(d, o + 10 + 2 * nc)? as usize;
        let mut p = o + 12 + 2 * nc + ilen;
        let mut flags = Vec::with_capacity(np);
        while flags.len() < np {
            let f = u8_at(d, p)?;
            p += 1;
            flags.push(f);
            if f & 8 != 0 {
                let r = u8_at(d, p)?;
                p += 1;
                for _ in 0..r {
                    flags.push(f);
                }
            }
        }
        flags.truncate(np);
        let mut read_coords = |short: u8, same: u8| -> Option<Vec<f32>> {
            let mut v = 0i32;
            let mut vs = Vec::with_capacity(np);
            for &f in &flags {
                if f & short != 0 {
                    let dv = i32::from(u8_at(d, p)?);
                    p += 1;
                    v += if f & same != 0 { dv } else { -dv };
                } else if f & same == 0 {
                    v += i32::from(i16_at(d, p)?);
                    p += 2;
                }
                vs.push(v as f32);
            }
            Some(vs)
        };
        let xs = read_coords(2, 16)?;
        let ys = read_coords(4, 32)?;
        let mut st = 0;
        for &en in &ends {
            if en < st || en >= np {
                return None; // 輪郭の終点が単調増加でない = 壊れている
            }
            let pts: Vec<(f32, f32, bool)> = (st..=en)
                .filter_map(|i| Some((*xs.get(i)?, *ys.get(i)?, flags.get(i)? & 1 != 0)))
                .collect();
            st = en + 1;
            let c = flatten(&pts);
            if c.len() >= 2 {
                out.push(c.into_iter().map(|(x, y)| tf(x, y)).collect());
            }
        }
        Some(())
    }

    fn composite_glyph(&self, o: usize, m: [f32; 6], tf: &dyn Fn(f32, f32) -> Pt, out: &mut Vec<Vec<Pt>>, depth: u32) -> Option<()> {
        let d = &self.data;
        let mut p = o + 10;
        for _ in 0..64 {
            let fl = u16_at(d, p)?;
            let gi = u16_at(d, p + 2)?;
            p += 4;
            let (a1, a2) = if fl & 1 != 0 {
                let r = (f32::from(i16_at(d, p)?), f32::from(i16_at(d, p + 2)?));
                p += 4;
                r
            } else {
                let r = (f32::from(u8_at(d, p)? as i8), f32::from(u8_at(d, p + 1)? as i8));
                p += 2;
                r
            };
            let f2 = |o: usize| i16_at(d, o).map(|v| f32::from(v) / 16384.0);
            let mut s = [1.0, 0.0, 0.0, 1.0];
            if fl & 8 != 0 {
                s[0] = f2(p)?;
                s[3] = s[0];
                p += 2;
            } else if fl & 0x40 != 0 {
                s[0] = f2(p)?;
                s[3] = f2(p + 2)?;
                p += 4;
            } else if fl & 0x80 != 0 {
                s = [f2(p)?, f2(p + 2)?, f2(p + 4)?, f2(p + 6)?];
                p += 8;
            }
            // 引数はオフセットとして扱う (点合わせ指定は未対応)
            let (dx, dy) = tf(a1, a2);
            let sub = [
                m[0] * s[0] + m[2] * s[1],
                m[1] * s[0] + m[3] * s[1],
                m[0] * s[2] + m[2] * s[3],
                m[1] * s[2] + m[3] * s[3],
                dx,
                dy,
            ];
            self.glyph(gi, sub, out, depth + 1)?;
            if fl & 0x20 == 0 {
                return Some(());
            }
        }
        Some(())
    }
}

/// 2次ベジエを含む輪郭を折れ線へ
fn flatten(pts: &[(f32, f32, bool)]) -> Vec<Pt> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    // 曲線上 (on) の点から始める。全部 off なら最初の2点の中点から。
    let (start, mut seq): (Pt, Vec<(f32, f32, bool)>) = match pts.iter().position(|p| p.2) {
        Some(i) => ((pts[i].0, pts[i].1), (1..=n).map(|k| pts[(i + k) % n]).collect()),
        None => {
            let mid = ((pts[0].0 + pts[1].0) / 2.0, (pts[0].1 + pts[1].1) / 2.0);
            let mut s: Vec<_> = pts[1..].to_vec();
            s.push(pts[0]);
            s.push((mid.0, mid.1, true));
            (mid, s)
        }
    };
    if seq.last().is_some_and(|p| !p.2) {
        seq.push((start.0, start.1, true));
    }
    let mut out = vec![start];
    let mut cur = start;
    let mut ctrl: Option<Pt> = None;
    for (x, y, on) in seq {
        if on {
            match ctrl.take() {
                Some(c) => quad(&mut out, cur, c, (x, y)),
                None => out.push((x, y)),
            }
            cur = (x, y);
        } else if let Some(c) = ctrl {
            let mid = ((c.0 + x) / 2.0, (c.1 + y) / 2.0);
            quad(&mut out, cur, c, mid);
            cur = mid;
            ctrl = Some((x, y));
        } else {
            ctrl = Some((x, y));
        }
    }
    out
}

fn quad(out: &mut Vec<Pt>, a: Pt, c: Pt, b: Pt) {
    const N: usize = 6;
    for k in 1..=N {
        let t = k as f32 / N as f32;
        let u = 1.0 - t;
        out.push((u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1));
    }
}

/// 日本語 TrueType フォントを探して読み込む。環境変数 PD_FONT が最優先。
pub fn find_japanese_font() -> Result<Font> {
    let mut candidates: Vec<String> = std::env::var("PD_FONT").into_iter().collect();
    candidates.extend(
        [
            // Linux
            "/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf",
            "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
            "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
            // macOS
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
            // Windows
            "C:\\Windows\\Fonts\\YuGothM.ttc",
            "C:\\Windows\\Fonts\\msgothic.ttc",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    let user_choice = std::env::var("PD_FONT").ok();
    let mut problems = Vec::new();
    for c in &candidates {
        let path = Path::new(c);
        let chosen_by_user = user_choice.as_deref() == Some(c.as_str());
        if !path.exists() {
            if chosen_by_user {
                eprintln!("警告: PD_FONT のファイルがありません: {c} (ほかのフォントを探します)");
            }
            continue;
        }
        match Font::load(path) {
            Ok(f) => {
                // 日本語が描けるかも確認する
                if f.glyph_id('光') != 0 && f.glyph_id('A') != 0 {
                    return Ok(f);
                }
                problems.push(format!("{c}: 日本語の文字がありません"));
            }
            Err(e) => problems.push(e.to_string()),
        }
        if chosen_by_user {
            if let Some(p) = problems.last() {
                eprintln!("警告: PD_FONT のフォントが使えません: {p} (ほかのフォントを探します)");
            }
        }
    }
    let mut msg = String::from("日本語の TrueType フォントが見つかりません。\n  PD_FONT=/path/to/font.ttf で指定してください。");
    for p in problems {
        msg.push_str("\n  - ");
        msg.push_str(&p);
    }
    Err(AppError::Font(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_rejected_without_panic() {
        let inputs: Vec<Vec<u8>> = vec![
            vec![],
            vec![0; 3],
            b"ttcf".to_vec(),
            vec![0xFF; 64],
            {
                // テーブル数だけ大きく、中身が無い
                let mut v = vec![0, 1, 0, 0, 0xFF, 0xFF];
                v.resize(40, 0);
                v
            },
        ];
        for d in inputs {
            assert!(Font::parse(d).is_err());
        }
    }

    #[test]
    fn truncated_real_font_never_panics() {
        let path = Path::new("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf");
        let Ok(full) = std::fs::read(path) else { return }; // フォントが無い環境では省略
        for cut in [100usize, 1_000, 50_000, full.len() / 2, full.len() - 1] {
            if let Ok(f) = Font::parse(full[..cut.min(full.len())].to_vec()) {
                for ch in "光検出器 APD p⁺ ①〈M〉".chars() {
                    let _ = f.outline(ch);
                }
            }
        }
    }

    #[test]
    fn real_font_has_japanese() {
        let path = Path::new("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf");
        let Ok(f) = Font::load(path) else { return };
        assert!(f.outline('光').is_some_and(|o| !o.contours.is_empty()));
        assert!(f.outline('\u{10FFFF}').is_none());
    }
}
