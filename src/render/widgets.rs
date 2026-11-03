//! 画面部品: デバイス断面図・電界・電流グラフ・凡例・見出し・字幕・統計パネル。
//! シーンはこれらを並べて1フレームを作る。

#![allow(clippy::too_many_arguments)]

use super::canvas::{Align, Canvas};
use super::raster::Pt;
use super::theme::*;
use crate::physics::device::{Layer, LayerKind};
use crate::physics::sim::{Sim, FLASH_LIFE};
use crate::physics::stats::gain_stats;

/// 画面上の長方形。デバイスの (横位置 0..1, 深さ 0..1) をここに写す。
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn px(&self, u: f32) -> f32 {
        self.x + u * self.w
    }
    pub fn py(&self, d: f32) -> f32 {
        self.y + d * self.h
    }
}

fn layer_style(l: &Layer) -> (&'static str, &'static str, Rgb) {
    match l.kind {
        LayerKind::PPlus => ("p⁺", "", (120, 60, 72)),
        LayerKind::Intrinsic => ("i 層", "(空乏層・吸収)", (44, 52, 74)),
        LayerKind::Pi => ("π 層", "(吸収層)", (44, 52, 74)),
        LayerKind::PMult => ("p 層", "(増倍層)", (88, 62, 70)),
        LayerKind::NPlus => ("n⁺", "", (50, 78, 120)),
    }
}

/// デバイスの断面図と、その中の光子・キャリア・フラッシュ
pub fn device(c: &mut Canvas, s: &Sim, r: Rect, frame: u32, labels: bool) {
    let spec = s.spec();
    for l in &spec.layers {
        let (name, sub, col) = layer_style(l);
        c.rect(r.x, r.py(l.d0), r.w, r.py(l.d1) - r.py(l.d0), col, 1.0);
        if labels {
            let ym = (r.py(l.d0) + r.py(l.d1)) / 2.0;
            if sub.is_empty() {
                c.text(name, r.x - 14.0, ym + 8.0, 20, FG, 1.0, true, Align::R);
            } else {
                c.text(name, r.x - 14.0, ym, 20, FG, 1.0, true, Align::R);
                c.text(sub, r.x - 14.0, ym + 22.0, 15, DIM, 1.0, false, Align::R);
            }
        }
    }
    if let Some((m0, m1)) = spec.mult {
        let pulse = 0.25 + 0.15 * (frame as f32 * 0.15).sin();
        let (y0, h) = (r.py(m0), r.py(m1) - r.py(m0));
        c.rect(r.x, y0, r.w, h, IONIZE, 0.08 + pulse * 0.2);
        c.rect_outline(r.x, y0, r.w, h, IONIZE, 0.6, 1.5);
        if labels {
            c.text("増倍領域 (高電界)", r.x + r.w - 8.0, y0 - 6.0, 15, IONIZE, 0.9, true, Align::R);
        }
    }
    // 電極 (上: 光の入る窓つき, 下: 全面) と反射防止膜
    c.rect(r.x, r.y - 8.0, r.w * 0.13, 8.0, GOLD, 1.0);
    c.rect(r.x + r.w * 0.87, r.y - 8.0, r.w * 0.13, 8.0, GOLD, 1.0);
    c.rect(r.x + r.w * 0.13, r.y - 3.0, r.w * 0.74, 3.0, (110, 130, 170), 0.7);
    c.rect(r.x, r.y + r.h, r.w, 10.0, GOLD, 1.0);
    if labels {
        c.text("p側電極  −V", r.x, r.y - 16.0, 15, GOLD, 1.0, false, Align::L);
        c.text("n側電極  +V   (逆バイアス)", r.x, r.y + r.h + 32.0, 15, GOLD, 1.0, false, Align::L);
    }

    let sc = r.w / 400.0;
    for p in s.photons() {
        photon(c, r.px(p.x), r.py(p.d), sc, frame);
    }
    for f in s.flashes() {
        let k = f.t / FLASH_LIFE;
        let (col, r0) = if f.ion { (IONIZE, 4.0) } else { (PHOTON, 6.0) };
        let (x, y) = (r.px(f.x), r.py(f.d));
        c.ring(x, y, (r0 + 26.0 * k) * sc, col, (1.0 - k) * 0.9, 2.5 * sc);
        if f.t < 6.0 {
            c.circle(x, y, (10.0 - f.t) * sc, WHITE, 0.35 * (1.0 - f.t / 6.0));
        }
    }
    for k in s.carriers() {
        carrier(c, r.px(k.x), r.py(k.d), 8.0 * sc, k.elec);
    }
}

/// 波線の矢印で光子を描く (先端が (x, y))
fn photon(c: &mut Canvas, x: f32, y: f32, sc: f32, frame: u32) {
    let len = 46.0 * sc;
    let pts: Vec<Pt> = (0..=24)
        .map(|k| {
            let t = k as f32 / 24.0;
            (x + 5.0 * sc * (t * 14.0 + frame as f32 * 0.6).sin(), y - len * (1.0 - t))
        })
        .collect();
    c.polyline(&pts, PHOTON, 0.35, 7.0 * sc);
    c.polyline(&pts, PHOTON, 1.0, 2.5 * sc);
    c.polygon(&[(x, y + 9.0 * sc), (x - 6.0 * sc, y - 2.0 * sc), (x + 6.0 * sc, y - 2.0 * sc)], PHOTON, 1.0);
}

/// 電子 (−) / 正孔 (+) の丸
fn carrier(c: &mut Canvas, x: f32, y: f32, r: f32, elec: bool) {
    let col = if elec { ELEC } else { HOLE };
    c.circle(x, y, r * 1.375, col, 0.25);
    c.circle(x, y, r, col, 1.0);
    let lw = r * 0.225;
    c.line(x - r * 0.55, y, x + r * 0.55, y, WHITE, 1.0, lw);
    if !elec {
        c.line(x, y - r * 0.55, x, y + r * 0.55, WHITE, 1.0, lw);
    }
}

/// デバイスの右に電界強度の分布を描く
pub fn field_profile(c: &mut Canvas, s: &Sim, r: Rect) {
    let x0 = r.x + r.w + 50.0;
    let wmax = 120.0;
    c.text("電界強度 E", x0 - 4.0, r.y - 16.0, 15, DIM, 1.0, false, Align::L);
    c.line(x0, r.y, x0, r.y + r.h, DIM, 0.6, 1.0);
    let n = 200;
    let curve: Vec<Pt> = (0..=n)
        .map(|k| {
            let d = k as f32 / n as f32;
            (x0 + s.spec().field(d) * wmax, r.py(d))
        })
        .collect();
    let mut area = vec![(x0, r.y)];
    area.extend_from_slice(&curve);
    area.push((x0, r.y + r.h));
    c.polygon(&area, FIELD, 0.18);
    c.polyline(&curve, FIELD, 1.0, 2.0);
    // 電界の向き (n → p)
    let ax = x0 + wmax + 14.0;
    c.line(ax, r.py(0.6), ax, r.py(0.3), DIM, 0.8, 1.5);
    c.polygon(&[(ax, r.py(0.3) - 8.0), (ax - 5.0, r.py(0.3)), (ax + 5.0, r.py(0.3))], DIM, 0.8);
    c.text("E", ax + 8.0, r.py(0.47), 14, DIM, 1.0, false, Align::L);
}

/// 電流の時間変化グラフ (右端が現在)
pub fn current_graph(c: &mut Canvas, s: &Sim, r: Rect, ymax: f32, title: &str, col: Rgb) {
    let ymax = if ymax > 0.0 { ymax } else { 1.0 };
    c.rect(r.x, r.y, r.w, r.h, PANEL, 1.0);
    for k in 1..4 {
        let yy = r.y + r.h * k as f32 / 4.0;
        c.line(r.x, yy, r.x + r.w, yy, GRID, 0.6, 1.0);
    }
    c.rect_outline(r.x, r.y, r.w, r.h, FRAME, 1.0, 1.0);
    c.text(title, r.x, r.y - 10.0, 17, FG, 1.0, true, Align::L);
    c.text("時間 →", r.x + r.w, r.y + r.h + 20.0, 13, DIM, 1.0, false, Align::R);
    let hist = s.history();
    let n = hist.len().max(2);
    let line: Vec<Pt> = hist
        .iter()
        .enumerate()
        .map(|(k, v)| (r.x + r.w * k as f32 / (n - 1) as f32, r.y + r.h - (v / ymax).clamp(0.0, 1.0) * (r.h - 4.0)))
        .collect();
    let mut area = line.clone();
    area.push((r.x + r.w, r.y + r.h));
    area.push((r.x, r.y + r.h));
    c.polygon(&area, col, 0.22);
    c.polyline(&line, col, 1.0, 2.0);
    if hist.iter().any(|v| *v > ymax) {
        c.text("▲ 目盛外", r.x + r.w - 6.0, r.y + 18.0, 13, IONIZE, 1.0, false, Align::R);
    }
}

pub fn legend(c: &mut Canvas, x: f32, y: f32) {
    carrier(c, x, y - 6.0, 7.0, true);
    c.text("電子", x + 14.0, y, 15, FG, 1.0, false, Align::L);
    let x = x + 70.0;
    carrier(c, x, y - 6.0, 7.0, false);
    c.text("正孔", x + 14.0, y, 15, FG, 1.0, false, Align::L);
    let x = x + 70.0;
    let pts: Vec<Pt> = (0..=12).map(|k| (x - 10.0 + k as f32 * 1.8, y - 6.0 + 3.5 * (k as f32 * 1.4).sin())).collect();
    c.polyline(&pts, PHOTON, 1.0, 2.0);
    c.text("光子", x + 18.0, y, 15, FG, 1.0, false, Align::L);
    let x = x + 76.0;
    c.ring(x, y - 6.0, 7.0, IONIZE, 1.0, 2.0);
    c.text("衝突電離", x + 14.0, y, 15, FG, 1.0, false, Align::L);
}

/// 画面上部の見出し
pub fn header(c: &mut Canvas, title: &str, sub: &str) {
    c.text(title, 40.0, 52.0, 32, FG, 1.0, true, Align::L);
    let tw = c.text_width(title, 32, true);
    c.text(sub, 40.0 + tw + 18.0, 50.0, 18, DIM, 1.0, false, Align::L);
    c.line(40.0, 70.0, WIDTH as f32 - 40.0, 70.0, GRID, 1.0, 1.0);
}

/// 右上の小さな札 (「スロー再生」など)
pub fn badge(c: &mut Canvas, text: &str, col: Rgb) {
    let (x, y) = (WIDTH as f32 - 40.0, 50.0);
    let w = c.text_width(text, 15, true) + 20.0;
    c.rect(x - w, y - 20.0, w, 28.0, col, 0.18);
    c.rect_outline(x - w, y - 20.0, w, 28.0, col, 0.8, 1.2);
    c.text(text, x - 10.0, y, 15, col, 1.0, true, Align::R);
}

/// 字幕: (開始フレーム, 終了フレーム, 文) の中から今の文をフェード付きで出す
pub fn captions<S: AsRef<str>>(c: &mut Canvas, frame: u32, list: &[(u32, u32, S)]) {
    c.rect(0.0, 618.0, WIDTH as f32, 102.0, (8, 11, 20), 0.85);
    for (s, e, t) in list {
        if (*s..*e).contains(&frame) {
            let a = ((frame - s) as f32 / 10.0).min(1.0).min((e - frame) as f32 / 8.0);
            c.text(t.as_ref(), WIDTH as f32 / 2.0, 677.0, 27, FG, a, false, Align::C);
        }
    }
}

/// 名前と値の行を縦に並べる
pub fn value_rows(c: &mut Canvas, x: f32, y: f32, w: f32, step: f32, size: u32, rows: &[(&str, String, Rgb)]) {
    for (k, (name, v, col)) in rows.iter().enumerate() {
        let yy = y + k as f32 * step;
        c.text(name, x, yy, size, DIM, 1.0, false, Align::L);
        c.text(v, x + w, yy, size + 3, *col, 1.0, true, Align::R);
    }
}

/// 光子数・電子数・(電離回数)・M = 電子 ÷ 光子
pub fn count_panel(c: &mut Canvas, s: &Sim, x: f32, y: f32, show_ionizations: bool) {
    let n = s.counts();
    let mut rows = vec![("吸収した光子", n.absorbed.to_string(), PHOTON), ("集めた電子", n.collected.to_string(), ELEC)];
    if show_ionizations {
        rows.push(("衝突電離の回数", n.ionizations.to_string(), IONIZE));
    }
    value_rows(c, x, y, 260.0, 34.0, 19, &rows);
    let yy = y + rows.len() as f32 * 34.0;
    c.line(x, yy - 20.0, x + 260.0, yy - 20.0, GRID, 1.0, 1.0);
    // 「集めた電子 ÷ 吸収した光子」だと、まだ走っている電子や増倍途中の光子のせいでずれるので、
    // 増倍が終わって M が確定した光子だけで平均する
    let g = s.recent_gains();
    c.text("利得 M (光子1個あたり)", x, yy + 8.0, 19, DIM, 1.0, false, Align::L);
    let m = if g.is_empty() { "—".to_string() } else { format!("{:.1}", g.iter().map(|&v| f64::from(v)).sum::<f64>() / g.len() as f64) };
    c.text(&m, x + 260.0, yy + 10.0, 28, FG, 1.0, true, Align::R);
    let note = if g.is_empty() { "増倍が終わると確定".to_string() } else { format!("M が確定した光子 {} 個の平均", g.len()) };
    c.text(&note, x, yy + 32.0, 12, DIM, 1.0, false, Align::L);
}

/// 光子ごとの利得の統計 (平均・σ・F) と、直近 20 光子の M の棒グラフ
pub fn gain_panel(c: &mut Canvas, s: &Sim, x: f32, y: f32, col: Rgb) {
    let st = gain_stats(s.gains());
    let fmt = |f: Option<f64>, d: usize| f.map_or("—".to_string(), |v| format!("{v:.d$}"));
    value_rows(
        c,
        x,
        y,
        280.0,
        27.0,
        16,
        &[
            ("M が確定した光子", s.gains().len().to_string(), PHOTON),
            ("平均利得 〈M〉", fmt(st.map(|t| t.mean), 1), FG),
            ("ゆらぎ σ", fmt(st.map(|t| t.sd), 1), FG),
            ("過剰雑音係数 F", fmt(st.map(|t| t.excess_noise), 2), IONIZE),
        ],
    );
    const N: usize = 20;
    const TOP: f32 = 80.0; // これを超える M は上に ▲
    let (by, bh, bw) = (y + 148.0, 40.0, 280.0 / N as f32);
    c.text("光子ごとの M (直近20個, 1本=1光子)", x, y + 100.0, 12, DIM, 1.0, false, Align::L);
    c.line(x, by, x + 280.0, by, FRAME, 1.0, 1.0);
    let g = s.gains();
    for (i, &m) in g[g.len().saturating_sub(N)..].iter().enumerate() {
        let bx = x + i as f32 * bw;
        let h = (m as f32 / TOP).min(1.0) * bh;
        c.rect(bx + 1.5, by - h.max(1.5), bw - 3.0, h.max(1.5), col, 0.9);
        if m as f32 > TOP {
            c.polygon(&[(bx + bw / 2.0, by - bh - 7.0), (bx + 3.0, by - bh - 1.0), (bx + bw - 3.0, by - bh - 1.0)], IONIZE, 1.0);
        }
    }
}
