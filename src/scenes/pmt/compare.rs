//! PMT シーン: PMT (〈M〉≈16384) と APD (〈M〉≈70) の増倍ゆらぎの比較
//!
//! 平均増倍率が 200 倍以上違うので、横軸を M/〈M〉 (平均で割った値) にそろえて分布の形を比べる。

use crate::physics::device::{DeviceSpec, APD70_ALPHA_E, APD_ALPHA_RATIO};
use crate::physics::pmt::{precompute_pmt_gains, PmtSpec};
use crate::physics::sim::VE;
use crate::physics::sim::VH;
use crate::physics::stats::{gain_stats, precompute_gains_for, GainStats};
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::{Align, Canvas};
use crate::scenes::{edge_fade, Scene};

/// 事前計算の回数 (APD は裾が長く推定がぶれやすいので多めに)
pub const PMT_TRIALS: usize = 20_000;
pub const APD_TRIALS: usize = 20_000;
/// 横軸 M/〈M〉 の範囲とビン幅
const X_MAX: f64 = 4.0;
const BIN: f64 = 0.1;
const FRAMES: u32 = 540;

struct Dist {
    stats: GainStats,
    /// 0..X_MAX の密度ヒストグラム (M/〈M〉 の確率密度)
    density: Vec<f64>,
    /// X_MAX を超えた割合
    over: f64,
    /// 平均の半分未満の割合
    below_half: f64,
}

fn summarize(v: &[u32]) -> Option<Dist> {
    let stats = gain_stats(v)?;
    let nb = (X_MAX / BIN).round() as usize;
    let mut h = vec![0.0; nb];
    let (mut over, mut below) = (0usize, 0usize);
    for &m in v {
        let r = f64::from(m) / stats.mean;
        if r < 0.5 {
            below += 1;
        }
        let i = (r / BIN).floor() as usize;
        match h.get_mut(i) {
            Some(c) => *c += 1.0,
            None => over += 1,
        }
    }
    let n = v.len() as f64;
    Some(Dist { stats, density: h.into_iter().map(|c| c / (n * BIN)).collect(), over: over as f64 / n, below_half: below as f64 / n })
}

pub struct PmtVsApdScene {
    pmt: Option<Dist>,
    apd: Option<Dist>,
    pmt_theory: f64,
    apd_theory: f64,
    k_eff: f64,
}

impl PmtVsApdScene {
    pub fn new() -> Self {
        eprintln!("  (PMT {PMT_TRIALS} 回と APD {APD_TRIALS} 回を事前計算しています…)");
        let spec = PmtSpec::standard();
        let pmt = summarize(&precompute_pmt_gains(&spec, PMT_TRIALS, 0x5EED_9417));
        let apd = summarize(&precompute_gains_for(&DeviceSpec::apd_with_alpha(APD70_ALPHA_E), APD_TRIALS, 0x5EED_0A70));
        // 距離あたりの実効的な電離率の比 k = (α_h/v_h)/(α_e/v_e) (正孔が遅い分だけ大きくなる)
        let k_eff = f64::from(VE / VH / APD_ALPHA_RATIO);
        let m = apd.as_ref().map_or(70.0, |d| d.stats.mean);
        let apd_theory = k_eff * m + (1.0 - k_eff) * (2.0 - 1.0 / m);
        PmtVsApdScene { pmt, apd, pmt_theory: spec.excess_noise_theory(), apd_theory, k_eff }
    }

    fn draw_dist(c: &mut Canvas, d: &Dist, r: Rect, y_max: f64, grow: f32, col: Rgb) {
        if grow <= 0.0 {
            return;
        }
        let base = r.y + r.h;
        let xw = r.w / X_MAX as f32;
        let mut pts: Vec<Pt> = vec![(r.x, base)];
        for (i, &v) in d.density.iter().enumerate() {
            let y = base - ((v / y_max) as f32 * grow).min(1.0) * r.h;
            let x0 = r.x + i as f32 * BIN as f32 * xw;
            pts.push((x0, y));
            pts.push((x0 + BIN as f32 * xw, y));
        }
        pts.push((r.x + r.w, base));
        c.polygon(&pts, col, 0.28);
        c.polyline(&pts[1..pts.len() - 1], col, 1.0, 2.2);
    }
}

fn fmt_opt(d: &Option<Dist>, f: impl Fn(&Dist) -> String) -> String {
    d.as_ref().map_or("—".to_string(), f)
}

impl Scene for PmtVsApdScene {
    fn title(&self) -> &'static str {
        "PMT と APD の比較"
    }
    fn frames(&self) -> u32 {
        FRAMES
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        c.clear();
        widgets::header(c, "増倍のゆらぎ: PMT と APD", &format!("平均増倍率 PMT {:.0} (設計値)、APD 約 70", PmtSpec::standard().mean_gain()));

        // 分布 (横軸 M/〈M〉)
        let r = Rect::new(90.0, 130.0, 740.0, 380.0);
        let y_max = [&self.pmt, &self.apd]
            .iter()
            .filter_map(|d| d.as_ref())
            .flat_map(|d| d.density.iter().copied())
            .fold(0.0f64, f64::max)
            .max(1e-9)
            * 1.08;
        c.rect(r.x, r.y, r.w, r.h, PANEL, 1.0);
        for k in 0..=8 {
            let x = r.x + k as f32 * r.w / 8.0;
            c.line(x, r.y, x, r.y + r.h, GRID, 0.5, 1.0);
            c.text(&format!("{}", k as f32 * 0.5), x, r.y + r.h + 20.0, 13, DIM, 1.0, false, Align::C);
        }
        c.text("M / 〈M〉 (平均増倍率で割った値)", r.x + r.w / 2.0, r.y + r.h + 44.0, 15, FG, 1.0, false, Align::C);
        c.text("確率密度", r.x - 10.0, r.y - 12.0, 13, DIM, 1.0, false, Align::R);
        let ease = |t: f32| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
        if let Some(d) = &self.apd {
            Self::draw_dist(c, d, r, y_max, ease((f as f32 - 30.0) / 45.0), IONIZE);
        }
        if let Some(d) = &self.pmt {
            Self::draw_dist(c, d, r, y_max, ease(f as f32 / 45.0), ELEC);
        }
        let x1 = r.x + r.w / X_MAX as f32;
        c.dashed_vline(x1, r.y, r.y + r.h, FG, 0.7, 1.2);
        c.text("平均", x1 + 6.0, r.y + 18.0, 13, FG, 0.9, false, Align::L);
        c.rect_outline(r.x, r.y, r.w, r.h, FRAME, 1.0, 1.0);
        // 凡例
        c.rect(r.x + r.w - 260.0, r.y + 14.0, 16.0, 12.0, ELEC, 0.9);
        c.text(&format!("PMT ({} 段、各段 {})", PmtSpec::standard().stages, PmtSpec::standard().delta), r.x + r.w - 238.0, r.y + 25.0, 14, FG, 1.0, false, Align::L);
        c.rect(r.x + r.w - 260.0, r.y + 38.0, 16.0, 12.0, IONIZE, 0.9);
        c.text("APD (〈M〉≈ 70)", r.x + r.w - 238.0, r.y + 49.0, 14, FG, 1.0, false, Align::L);
        if f >= 75 {
            let over = format!(
                "4 を超える割合: PMT {}、APD {}",
                fmt_opt(&self.pmt, |d| format!("{:.1}%", d.over * 100.0)),
                fmt_opt(&self.apd, |d| format!("{:.1}%", d.over * 100.0))
            );
            c.text(&over, r.x + r.w - 10.0, r.y + 76.0, 12, DIM, 1.0, false, Align::R);
        }

        // 右: 比較表
        let (tx, c1, c2) = (870.0, 1120.0, 1235.0);
        let a = ((f as f32 - 80.0) / 20.0).clamp(0.0, 1.0);
        c.text("PMT", c1, 150.0, 16, ELEC, a, true, Align::R);
        c.text("APD", c2, 150.0, 16, IONIZE, a, true, Align::R);
        c.line(tx, 160.0, c2, 160.0, GRID, a, 1.0);
        let rows: [(&str, String, String); 7] = [
            ("平均増倍率 〈M〉", fmt_opt(&self.pmt, |d| format!("{:.0}", d.stats.mean)), fmt_opt(&self.apd, |d| format!("{:.1}", d.stats.mean))),
            ("中央値 ÷ 平均", fmt_opt(&self.pmt, |d| format!("{:.2}", f64::from(d.stats.median) / d.stats.mean)), fmt_opt(&self.apd, |d| format!("{:.2}", f64::from(d.stats.median) / d.stats.mean))),
            ("平均の半分未満", fmt_opt(&self.pmt, |d| format!("{:.0}%", d.below_half * 100.0)), fmt_opt(&self.apd, |d| format!("{:.0}%", d.below_half * 100.0))),
            ("相対ゆらぎ σ/〈M〉", fmt_opt(&self.pmt, |d| format!("{:.2}", d.stats.sd / d.stats.mean)), fmt_opt(&self.apd, |d| format!("{:.2}", d.stats.sd / d.stats.mean))),
            ("過剰雑音係数 F", fmt_opt(&self.pmt, |d| format!("{:.2}", d.stats.excess_noise)), fmt_opt(&self.apd, |d| format!("{:.2}", d.stats.excess_noise))),
            ("F の理論値", format!("{:.2}", self.pmt_theory), format!("{:.2}", self.apd_theory)),
            ("雑音の倍率 √F", fmt_opt(&self.pmt, |d| format!("{:.2}", d.stats.excess_noise.sqrt())), fmt_opt(&self.apd, |d| format!("{:.2}", d.stats.excess_noise.sqrt()))),
        ];
        for (i, (name, p, q)) in rows.iter().enumerate() {
            let y = 192.0 + i as f32 * 38.0;
            let k = ((f as f32 - 90.0 - i as f32 * 8.0) / 15.0).clamp(0.0, 1.0);
            let hl = i == 4;
            c.text(name, tx, y, 15, if hl { FG } else { DIM }, k, hl, Align::L);
            c.text(p, c1, y, if hl { 20 } else { 17 }, ELEC, k, true, Align::R);
            c.text(q, c2, y, if hl { 20 } else { 17 }, IONIZE, k, true, Align::R);
        }
        let k = ((f as f32 - 160.0) / 15.0).clamp(0.0, 1.0);
        let spec = PmtSpec::standard();
        let last = spec.delta.powi(spec.stages as i32 - 1);
        c.text(&format!("理論値: PMT は 1 + 1/{} + … + 1/{last:.0} (ポアソン放出)", spec.delta), tx, 470.0, 12, DIM, k, false, Align::L);
        c.text(&format!("APD は McIntyre の式 (実効 k = {:.3})", self.k_eff), tx, 490.0, 12, DIM, k, false, Align::L);
        c.text(&format!("シミュレーション: 各 {PMT_TRIALS} 回"), tx, 510.0, 12, DIM, k, false, Align::L);
        c.text("APD の値が理論より小さいのは、時間を", tx, 536.0, 12, DIM, k, false, Align::L);
        c.text("区切って計算する近似のため (資料参照)", tx, 554.0, 12, DIM, k, false, Align::L);

        let (fp, fa) = (self.pmt.as_ref().map_or(0.0, |d| d.stats.excess_noise), self.apd.as_ref().map_or(0.0, |d| d.stats.excess_noise));
        widgets::captions(
            c,
            f,
            &[
                (0, 130, "平均増倍率が違うので、横軸を M/〈M〉 にそろえて分布の形を比べる".to_string()),
                (130, 260, "PMT は平均の周りにまとまる。APD は大半が平均より小さく、まれに何倍にもなる".to_string()),
                (260, 400, format!("過剰雑音係数 F: PMT {fp:.2}、APD {fa:.1} → 雑音は理想の √F 倍 ({:.2} 倍 と {:.1} 倍)", fp.sqrt(), fa.sqrt())),
                (400, 470, "PMT: 1 段目で平均 4 個に増えると、後の段のゆらぎは 1/4 ずつ小さく効く".to_string()),
                (470, FRAMES, "APD: 電子が少ないうちの偶然 (いつ電離するか) と、正孔の帰還が大きく効く".to_string()),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}
