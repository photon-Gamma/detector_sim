//! 符号付き面積の累積によるアンチエイリアス塗りつぶし (font-rs と同じ方式)。
//!
//! 輪郭の各辺が「右側すべてに寄与する面積」をバッファに足しておき、
//! 行ごとに左から累積和をとると各ピクセルの被覆率になる。
//! 穴 (逆向きの輪郭) は打ち消し合い、同じ向きの重なりは 1 で頭打ちにする。

pub type Pt = (f32, f32);

/// 被覆率を計算する作業領域 (毎回の確保を避けるため使い回す)
#[derive(Default)]
pub struct Coverage {
    buf: Vec<f32>,
    w: usize,
    h: usize,
}

/// バッファの1行は w+3 要素 (右端のはみ出し分の余白)
const PAD: usize = 3;

/// 範囲外の書き込みは無視する (パニックしない)
#[inline]
fn add(buf: &mut [f32], i: usize, v: f32) {
    if let Some(a) = buf.get_mut(i) {
        *a += v;
    }
}

impl Coverage {
    /// (x, y) の被覆率 0..1。範囲外は 0。
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x >= self.w || y >= self.h {
            return 0.0;
        }
        self.buf.get(y * (self.w + PAD) + x).copied().unwrap_or(0.0)
    }

    /// w×h の領域に contours (領域座標) を塗った被覆率を計算する。
    /// 非有限の点を含む輪郭は無視する。
    pub fn compute(&mut self, contours: &[Vec<Pt>], w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let stride = w + PAD;
        self.buf.clear();
        self.buf.resize(stride * h, 0.0);
        let wf = w as f32;
        for c in contours {
            if c.len() < 2 || c.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
                continue;
            }
            for (i, &p0) in c.iter().enumerate() {
                let p1 = c[(i + 1) % c.len()];
                // 左右のはみ出しは端に押し付けても、領域内の被覆率は変わらない
                self.line((p0.0.clamp(0.0, wf), p0.1), (p1.0.clamp(0.0, wf), p1.1));
            }
        }
        for row in self.buf.chunks_mut(stride) {
            let mut s = 0.0;
            for v in row.iter_mut() {
                s += *v;
                *v = s.abs().min(1.0);
            }
        }
    }

    fn line(&mut self, p0: Pt, p1: Pt) {
        if p0.1 == p1.1 {
            return;
        }
        let stride = self.w + PAD;
        let (dir, p0, p1) = if p0.1 < p1.1 { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
        let ys = p0.1.max(0.0);
        let ye = p1.1.min(self.h as f32);
        if ys >= ye {
            return;
        }
        let buf = &mut self.buf;
        let mut x = p0.0 + (ys - p0.1) * dxdy;
        let mut y = ys.floor() as usize;
        while (y as f32) < ye {
            let ls = y * stride;
            let dy = ((y + 1) as f32).min(ye) - (y as f32).max(ys);
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let x0floor = x0.floor();
            let x0i = x0floor.max(0.0) as usize;
            let x1ceil = x1.ceil();
            let x1i = x1ceil.max(0.0) as usize;
            if x1i <= x0i + 1 {
                // 1ピクセル内に収まる
                let xmf = 0.5 * (x + xnext) - x0floor;
                add(buf, ls + x0i, d - d * xmf);
                add(buf, ls + x0i + 1, d * xmf);
            } else {
                // 複数ピクセルにまたがる
                let s = 1.0 / (x1 - x0);
                let x0f = x0 - x0floor;
                let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
                let x1f = x1 - x1ceil + 1.0;
                let am = 0.5 * s * x1f * x1f;
                add(buf, ls + x0i, d * a0);
                if x1i == x0i + 2 {
                    add(buf, ls + x0i + 1, d * (1.0 - a0 - am));
                } else {
                    let a1 = s * (1.5 - x0f);
                    add(buf, ls + x0i + 1, d * (a1 - a0));
                    for xi in x0i + 2..x1i - 1 {
                        add(buf, ls + xi, d * s);
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    add(buf, ls + x1i - 1, d * (1.0 - a2 - am));
                }
                add(buf, ls + x1i, d * am);
            }
            x = xnext;
            y += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Pt> {
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    }

    #[test]
    fn full_and_half_pixels() {
        let mut c = Coverage::default();
        c.compute(&[square(1.0, 1.0, 3.0, 3.0)], 4, 4);
        assert!((c.get(1, 1) - 1.0).abs() < 1e-5);
        assert_eq!(c.get(0, 0), 0.0);
        c.compute(&[square(0.5, 0.0, 2.0, 1.0)], 4, 1);
        assert!((c.get(0, 0) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn hole_cancels() {
        let mut inner = square(1.0, 1.0, 3.0, 3.0);
        inner.reverse();
        let mut c = Coverage::default();
        c.compute(&[square(0.0, 0.0, 4.0, 4.0), inner], 4, 4);
        assert!(c.get(2, 2) < 1e-5);
        assert!((c.get(0, 0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hostile_coordinates_do_not_panic() {
        let mut c = Coverage::default();
        let bad = vec![
            vec![(f32::NAN, 0.0), (1.0, 1.0), (2.0, 3.0)],
            vec![(-1e30, -1e30), (1e30, 5.0), (3.0, 1e30)],
            vec![(0.0, 0.0), (1e-30, 1e-30)],
            vec![(10.0, 10.0)],
        ];
        for w in [0, 1, 7] {
            for h in [0, 1, 5] {
                c.compute(&bad, w, h);
            }
        }
    }
}
