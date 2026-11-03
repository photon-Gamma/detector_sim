//! 1フレーム分の画像 (RGB) と描画 API。
//!
//! 図形はすべて「輪郭 → 被覆率 → 色を混ぜる」の同じ流れで描く。
//! 座標が画面外・NaN でも安全に無視される。

// 描画関数は 位置・大きさ・色・不透明度 を並べて渡す形にしている
#![allow(clippy::too_many_arguments)]

use super::font::Font;
use super::raster::{Coverage, Pt};
use super::theme::Rgb;
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    L,
    C,
    R,
}

/// 文字サイズの範囲 (極端な値でメモリを使い過ぎないため)
const MIN_PX: u32 = 4;
const MAX_PX: u32 = 200;

struct Glyph {
    left: i32,
    top: i32,
    w: usize,
    h: usize,
    cov: Vec<f32>,
    adv: f32,
}

pub struct Canvas {
    w: usize,
    h: usize,
    buf: Vec<u8>,
    bg: Rgb,
    font: Font,
    glyphs: HashMap<(char, u32), Glyph>,
    warned: HashSet<char>,
    cov: Coverage,
}

fn bbox(pts: impl Iterator<Item = Pt>) -> Option<(f32, f32, f32, f32)> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for (x, y) in pts {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        b = Some(match b {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    b
}

/// 1画素に色 c を割合 k で混ぜる (範囲外は無視)
#[inline]
fn blend_into(buf: &mut [u8], w: usize, x: usize, y: usize, c: Rgb, k: f32) {
    let o = (y * w + x) * 3;
    if let Some(px) = buf.get_mut(o..o + 3) {
        let k = k.clamp(0.0, 1.0);
        for (v, t) in px.iter_mut().zip([c.0, c.1, c.2]) {
            *v = (f32::from(*v) + (f32::from(t) - f32::from(*v)) * k) as u8;
        }
    }
}

fn signed_area(c: &[Pt]) -> f32 {
    (0..c.len()).map(|i| {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        p.0 * q.1 - q.0 * p.1
    }).sum()
}

/// 向きを正にそろえる (重なった図形が打ち消し合わず和集合になるように)
fn positive(mut c: Vec<Pt>) -> Vec<Pt> {
    if signed_area(&c) < 0.0 {
        c.reverse();
    }
    c
}

fn circle_pts(x: f32, y: f32, r: f32) -> Vec<Pt> {
    let r = if r.is_finite() { r.clamp(0.0, 4096.0) } else { 0.0 };
    let n = ((r * 1.6) as usize).clamp(10, 96);
    (0..n).map(|k| {
        let t = k as f32 / n as f32 * TAU;
        (x + r * t.cos(), y + r * t.sin())
    }).collect()
}

impl Canvas {
    pub fn new(w: usize, h: usize, bg: Rgb, font: Font) -> Self {
        let (w, h) = (w.clamp(1, 8192), h.clamp(1, 8192));
        Canvas { w, h, buf: vec![0; w * h * 3], bg, font, glyphs: HashMap::new(), warned: HashSet::new(), cov: Coverage::default() }
    }

    /// RGB24 の画素列 (幅×高さ×3 バイト)
    pub fn pixels(&self) -> &[u8] {
        &self.buf
    }

    pub fn clear(&mut self) {
        let bg = [self.bg.0, self.bg.1, self.bg.2];
        for p in self.buf.chunks_exact_mut(3) {
            p.copy_from_slice(&bg);
        }
    }

    #[inline]
    fn blend(&mut self, x: usize, y: usize, c: Rgb, k: f32) {
        blend_into(&mut self.buf, self.w, x, y, c, k);
    }

    /// 汎用の塗り (スクリーン座標の輪郭群)
    pub fn fill(&mut self, contours: &[Vec<Pt>], c: Rgb, alpha: f32) {
        if alpha.is_nan() || alpha <= 0.0 {
            return;
        }
        let Some((x0, y0, x1, y1)) = bbox(contours.iter().flatten().copied()) else { return };
        let bx0 = x0.floor().clamp(0.0, self.w as f32) as usize;
        let by0 = y0.floor().clamp(0.0, self.h as f32) as usize;
        let bx1 = x1.ceil().clamp(0.0, self.w as f32) as usize;
        let by1 = y1.ceil().clamp(0.0, self.h as f32) as usize;
        if bx1 <= bx0 || by1 <= by0 {
            return;
        }
        let (bw, bh) = (bx1 - bx0, by1 - by0);
        let local: Vec<Vec<Pt>> = contours.iter().map(|c| c.iter().map(|p| (p.0 - bx0 as f32, p.1 - by0 as f32)).collect()).collect();
        let mut cov = std::mem::take(&mut self.cov);
        cov.compute(&local, bw, bh);
        for y in 0..bh {
            for x in 0..bw {
                let k = cov.get(x, y) * alpha;
                if k > 0.0 {
                    self.blend(bx0 + x, by0 + y, c, k);
                }
            }
        }
        self.cov = cov;
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb, a: f32) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.fill(&[vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]], c, a);
    }

    pub fn rect_outline(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb, a: f32, lw: f32) {
        let o = lw / 2.0;
        let outer = vec![(x - o, y - o), (x + w + o, y - o), (x + w + o, y + h + o), (x - o, y + h + o)];
        let mut inner = vec![(x + o, y + o), (x + w - o, y + o), (x + w - o, y + h - o), (x + o, y + h - o)];
        inner.reverse();
        self.fill(&[outer, inner], c, a);
    }

    pub fn circle(&mut self, x: f32, y: f32, r: f32, c: Rgb, a: f32) {
        self.fill(&[circle_pts(x, y, r)], c, a);
    }

    pub fn ring(&mut self, x: f32, y: f32, r: f32, c: Rgb, a: f32, lw: f32) {
        let outer = positive(circle_pts(x, y, r + lw / 2.0));
        let mut inner = positive(circle_pts(x, y, (r - lw / 2.0).max(0.0)));
        inner.reverse();
        self.fill(&[outer, inner], c, a);
    }

    /// 丸い端・丸い継ぎ目の太線
    pub fn polyline(&mut self, pts: &[Pt], c: Rgb, a: f32, lw: f32) {
        if pts.len() < 2 || lw.is_nan() || lw <= 0.0 {
            return;
        }
        let r = lw / 2.0;
        let mut cs = Vec::with_capacity(pts.len() * 2);
        for s in pts.windows(2) {
            let (p, q) = (s[0], s[1]);
            let (dx, dy) = (q.0 - p.0, q.1 - p.1);
            let l = (dx * dx + dy * dy).sqrt();
            if l.is_nan() || l <= 1e-4 {
                continue;
            }
            let (nx, ny) = (-dy / l * r, dx / l * r);
            cs.push(positive(vec![(p.0 + nx, p.1 + ny), (q.0 + nx, q.1 + ny), (q.0 - nx, q.1 - ny), (p.0 - nx, p.1 - ny)]));
        }
        if r > 0.9 {
            for p in pts {
                cs.push(positive(circle_pts(p.0, p.1, r)));
            }
        }
        self.fill(&cs, c, a);
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, lw: f32) {
        self.polyline(&[(x0, y0), (x1, y1)], c, a, lw);
    }

    /// 縦の破線
    pub fn dashed_vline(&mut self, x: f32, y0: f32, y1: f32, c: Rgb, a: f32, lw: f32) {
        let mut y = y0;
        let mut guard = 0;
        while y < y1 && guard < 1000 {
            self.line(x, y, x, (y + 6.0).min(y1), c, a, lw);
            y += 11.0;
            guard += 1;
        }
    }

    pub fn polygon(&mut self, pts: &[Pt], c: Rgb, a: f32) {
        self.fill(&[pts.to_vec()], c, a);
    }

    /// 全体を暗くする (フェード)。k=1 でそのまま、0 で真っ黒。
    pub fn fade(&mut self, k: f32) {
        if k.is_nan() || k >= 1.0 {
            return;
        }
        let k = k.max(0.0);
        for v in self.buf.iter_mut() {
            *v = (f32::from(*v) * k) as u8;
        }
    }

    // ------------------------------------------------ 文字
    fn ensure_glyph(&mut self, ch: char, px: u32) {
        if self.glyphs.contains_key(&(ch, px)) {
            return;
        }
        let g = self.make_glyph(ch, px);
        self.glyphs.insert((ch, px), g);
    }

    fn make_glyph(&mut self, ch: char, px: u32) -> Glyph {
        let pxf = px as f32;
        // 上付き + はフォントに無いことが多いので、+ を縮小して持ち上げる
        let (src, scale_k, lift, extra) = match ch {
            '⁺' => ('+', 0.62, 0.42 * pxf, 0.04 * pxf),
            _ => (ch, 1.0, 0.0, 0.0),
        };
        let empty = |adv: f32| Glyph { left: 0, top: 0, w: 0, h: 0, cov: Vec::new(), adv };
        let Some(ol) = self.font.outline(src) else {
            if self.warned.insert(ch) {
                eprintln!("警告: フォントに '{}' (U+{:04X}) がありません。空白で表示します。", ch, ch as u32);
            }
            return empty(pxf * 0.5);
        };
        let s = pxf / self.font.units_per_em() * scale_k;
        let adv = ol.advance * s + extra;
        // y を下向きに
        let cs: Vec<Vec<Pt>> = ol.contours.iter().map(|c| c.iter().map(|p| (p.0 * s, -p.1 * s - lift)).collect()).collect();
        let Some((x0, y0, x1, y1)) = bbox(cs.iter().flatten().copied()) else { return empty(adv) };
        let (l, t) = (x0.floor() as i32, y0.floor() as i32);
        let w = ((x1.ceil() as i32 - l).max(0) as usize + 1).min(4 * MAX_PX as usize);
        let h = ((y1.ceil() as i32 - t).max(0) as usize + 1).min(4 * MAX_PX as usize);
        let local: Vec<Vec<Pt>> = cs.iter().map(|c| c.iter().map(|p| (p.0 - l as f32, p.1 - t as f32)).collect()).collect();
        let mut cov = Coverage::default();
        cov.compute(&local, w, h);
        let mut out = vec![0.0; w * h];
        for y in 0..h {
            for x in 0..w {
                out[y * w + x] = cov.get(x, y);
            }
        }
        Glyph { left: l, top: t, w, h, cov: out, adv }
    }

    pub fn text_width(&mut self, s: &str, px: u32, bold: bool) -> f32 {
        let px = px.clamp(MIN_PX, MAX_PX);
        let mut w = 0.0;
        for ch in s.chars() {
            self.ensure_glyph(ch, px);
            w += self.glyphs.get(&(ch, px)).map_or(0.0, |g| g.adv);
        }
        w + if bold { 1.0 } else { 0.0 }
    }

    /// 文字列を描く。y はベースライン。太字は少し横にずらした重ね描き。
    pub fn text(&mut self, s: &str, x: f32, y: f32, px: u32, c: Rgb, a: f32, bold: bool, al: Align) {
        if a.is_nan() || a <= 0.0 || !x.is_finite() || !y.is_finite() {
            return;
        }
        let px = px.clamp(MIN_PX, MAX_PX);
        let tw = self.text_width(s, px, bold);
        let x = match al {
            Align::L => x,
            Align::C => x - tw / 2.0,
            Align::R => x - tw,
        };
        let offsets: &[f32] = if bold { &[0.0, (px as f32 / 28.0).max(0.7)] } else { &[0.0] };
        for &ox in offsets {
            let mut pen = x + ox;
            for ch in s.chars() {
                self.ensure_glyph(ch, px);
                let Some(g) = self.glyphs.get(&(ch, px)) else { continue };
                let (gx, gy) = (pen.round() as i32 + g.left, y.round() as i32 + g.top);
                for (j, row) in g.cov.chunks(g.w.max(1)).enumerate().take(g.h) {
                    for (i, &cv) in row.iter().enumerate() {
                        let k = cv * a;
                        let (xx, yy) = (gx + i as i32, gy + j as i32);
                        if k > 0.0 && xx >= 0 && yy >= 0 && (xx as usize) < self.w && (yy as usize) < self.h {
                            blend_into(&mut self.buf, self.w, xx as usize, yy as usize, c, k);
                        }
                    }
                }
                pen += g.adv;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::Path;

    fn canvas() -> Option<Canvas> {
        let f = Font::load(Path::new("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf")).ok()?;
        Some(Canvas::new(64, 48, (0, 0, 0), f))
    }

    #[test]
    fn drawing_outside_or_with_nan_is_harmless() {
        let Some(mut c) = canvas() else { return };
        c.clear();
        c.circle(f32::NAN, 10.0, 5.0, (255, 0, 0), 1.0);
        c.circle(-1e9, 1e9, 1e9, (255, 0, 0), 1.0);
        c.rect(-100.0, -100.0, 50.0, 50.0, (255, 0, 0), 1.0);
        c.polyline(&[(0.0, 0.0), (f32::INFINITY, 3.0), (5.0, 5.0)], (255, 0, 0), 1.0, 3.0);
        c.text("光検出器 ①〈M〉", -500.0, 2000.0, 100_000, (255, 255, 255), 1.0, true, Align::C);
        c.text("\u{10FFFF}", 10.0, 10.0, 0, (255, 255, 255), f32::NAN, false, Align::L);
        c.fade(f32::NAN);
        assert_eq!(c.pixels().len(), 64 * 48 * 3);
    }

    #[test]
    fn rect_is_painted() {
        let Some(mut c) = canvas() else { return };
        c.clear();
        c.rect(10.0, 10.0, 10.0, 10.0, (200, 100, 50), 1.0);
        let o = (15 * 64 + 15) * 3;
        assert_eq!(&c.pixels()[o..o + 3], &[200, 100, 50]);
    }
}
