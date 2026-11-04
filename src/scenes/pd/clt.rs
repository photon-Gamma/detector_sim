//! シーン 5: 平均利得の分布 (中心極限定理)
//!
//! N 個の光子の平均利得を BATCHES 回求め、N = 1, 10, 100, 1000 で分布を比べる。
//! - 形: N が大きいほど正規分布に近づくか (歪度 → 0)
//! - 幅: 標準偏差が σ/√N で小さくなるか (右の両対数グラフ)
//!
//! 光子ごとの利得は、すべて独立なシミュレーション結果 (重なりのない組に分けて平均)。

use crate::scenes::{edge_fade, Scene};
use crate::physics::stats::{batch_means, describe, gain_stats, precompute_gains, Describe};
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::{Align, Canvas};

/// 1つの N について平均を何回求めるか
pub const BATCHES: usize = 400;
/// ヒストグラムを描く N
const ROW_NS: [usize; 4] = [1, 10, 100, 1000];
/// 各行のビン幅 (N が大きいほど分布が細いので細かく)
const ROW_BINS: [f64; 4] = [2.0, 1.0, 0.5, 0.25];
/// 右の両対数グラフに打つ N
const PLOT_NS: [usize; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
/// 共通の横軸 (平均利得)
const X_MAX: f64 = 60.0;
const SEED: u64 = 0xC17;

/// 行の表示タイミング
const ROW_START: u32 = 15;
const ROW_STEP: u32 = 50;
const ROW_FILL: u32 = 60;
const PLOT_START: u32 = 250;

struct Row {
    n: usize,
    bin: f64,
    means: Vec<f64>,
    /// 全データでのビンごとの最大密度 (行の縦の目盛を決める)
    peak: f64,
}

pub struct CltScene {
    mu: f64,
    sigma: f64,
    rows: Vec<Row>,
    /// (N, 実測の標準偏差)
    plot: Vec<(usize, f64)>,
}

/// 正規分布の確率密度
fn normal_pdf(x: f64, mu: f64, sd: f64) -> f64 {
    if sd <= 0.0 {
        return 0.0;
    }
    let z = (x - mu) / sd;
    (-0.5 * z * z).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt())
}

/// 密度ヒストグラム (横軸 0..X_MAX、はみ出しは別に数える)
fn density(means: &[f64], bin: f64) -> (Vec<f64>, usize) {
    let nb = (X_MAX / bin).ceil() as usize;
    let mut h = vec![0.0; nb];
    let mut over = 0;
    for &m in means {
        let i = (m / bin).floor();
        if i >= 0.0 && (i as usize) < nb {
            if let Some(v) = h.get_mut(i as usize) {
                *v += 1.0;
            }
        } else {
            over += 1;
        }
    }
    let k = 1.0 / (means.len().max(1) as f64 * bin);
    (h.into_iter().map(|c| c * k).collect(), over)
}

impl CltScene {
    pub fn new() -> Self {
        let max_n = ROW_NS.iter().chain(PLOT_NS.iter()).copied().max().unwrap_or(1);
        eprintln!("  (平均の分布用に {} 光子ぶんを事前計算しています…)", BATCHES * max_n);
        let pool = precompute_gains(BATCHES * max_n, SEED);
        let (mu, sigma) = gain_stats(&pool).map_or((0.0, 0.0), |s| (s.mean, s.sd));
        let rows = ROW_NS
            .iter()
            .zip(ROW_BINS)
            .map(|(&n, bin)| {
                let means = batch_means(&pool, n, BATCHES);
                let (d, _) = density(&means, bin);
                let peak = d.iter().copied().fold(0.0, f64::max).max(normal_pdf(mu, mu, sigma / (n as f64).sqrt()));
                Row { n, bin, means, peak }
            })
            .collect();
        let plot = PLOT_NS.iter().filter_map(|&n| describe(&batch_means(&pool, n, BATCHES)).map(|d| (n, d.sd))).collect();
        CltScene { mu, sigma, rows, plot }
    }

    fn draw_row(&self, c: &mut Canvas, r: &Row, area: Rect, shown: usize, curve_a: f32) {
        let means = r.means.get(..shown.min(r.means.len())).unwrap_or(&[]);
        let x_of = |v: f64| area.x + (v / X_MAX) as f32 * area.w;
        c.rect(area.x, area.y, area.w, area.h, PANEL, 1.0);
        // 縦は全データの最大で固定 (データが増えるとバーが伸びる)
        let scale = if r.peak > 0.0 { (area.h as f64 - 12.0) / r.peak } else { 0.0 };
        let total = r.means.len().max(1) as f64;
        let (d, over) = density(means, r.bin);
        let base = area.y + area.h;
        let bw = (r.bin / X_MAX) as f32 * area.w;
        for (i, &v) in d.iter().enumerate() {
            // 途中経過は「最終的な総数に対する割合」で描く
            let hgt = (v * means.len() as f64 / total * scale) as f32;
            if hgt > 0.0 {
                c.rect(area.x + i as f32 * bw + 0.5, base - hgt, (bw - 1.0).max(1.0), hgt, HIST_BAR, 0.95);
            }
        }
        // 正規分布 N(μ, σ²/N)
        if curve_a > 0.0 {
            let sd = self.sigma / (r.n as f64).sqrt();
            let pts: Vec<Pt> = (0..=240)
                .map(|k| {
                    let x = X_MAX * k as f64 / 240.0;
                    (x_of(x), base - ((normal_pdf(x, self.mu, sd) * scale) as f32).min(area.h))
                })
                .collect();
            c.polyline(&pts, IONIZE, curve_a, 2.0);
        }
        if over > 0 {
            c.text(&format!("{X_MAX:.0} 超: {over} 回 →"), area.x + area.w - 6.0, area.y + 18.0, 12, HIST_OVER, 1.0, false, Align::R);
        }
        c.dashed_vline(x_of(self.mu), area.y, base, FG, 0.5, 1.0);
        c.rect_outline(area.x, area.y, area.w, area.h, FRAME, 1.0, 1.0);

        // 左の説明
        let lx = 40.0;
        c.text(&format!("N = {}", r.n), lx, area.y + 24.0, 20, FG, 1.0, true, Align::L);
        c.text(&format!("{}個の平均 × {}回", r.n, r.means.len()), lx, area.y + 44.0, 12, DIM, 1.0, false, Align::L);
        if let Some(Describe { sd, skew, .. }) = describe(means) {
            c.text(&format!("SD {sd:.2}  (理論 {:.2})", self.sigma / (r.n as f64).sqrt()), lx, area.y + 68.0, 13, FG, 1.0, false, Align::L);
            c.text(&format!("歪度 {skew:+.2}"), lx, area.y + 88.0, 13, if skew.abs() < 0.3 { FIELD } else { HOLE }, 1.0, false, Align::L);
            if r.n == 1 && sd < 0.95 * self.sigma {
                let note = format!("※ SD が理論より小さいのは、{} 個ではまれな大きい M を拾いきれないため", r.means.len());
                c.text(&note, area.x + area.w - 6.0, area.y + 36.0, 11, DIM, 1.0, false, Align::R);
            }
        }
    }

    fn draw_plot(&self, c: &mut Canvas, f: u32) {
        let p = Rect::new(935.0, 120.0, 290.0, 300.0);
        c.text("平均のばらつき SD と N", p.x, p.y - 18.0, 16, FG, 1.0, true, Align::L);
        c.rect(p.x, p.y, p.w, p.h, PANEL, 1.0);
        let (ly0, ly1) = ((0.5f64).log10(), (80.0f64).log10());
        let xp = |n: f64| p.x + (n.log10() / 3.0) as f32 * p.w;
        let yp = |sd: f64| p.y + p.h - ((sd.max(1e-9).log10() - ly0) / (ly1 - ly0)) as f32 * p.h;
        for t in [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0] {
            let y = yp(t);
            c.line(p.x, y, p.x + p.w, y, GRID, 0.6, 1.0);
            c.text(&format!("{t}"), p.x - 6.0, y + 4.0, 11, DIM, 1.0, false, Align::R);
        }
        for n in [1.0, 10.0, 100.0, 1000.0] {
            let x = xp(n);
            c.line(x, p.y, x, p.y + p.h, GRID, 0.6, 1.0);
            c.text(&format!("{n}"), x, p.y + p.h + 16.0, 11, DIM, 1.0, false, Align::C);
        }
        c.text("N (平均した光子数)", p.x + p.w / 2.0, p.y + p.h + 34.0, 12, DIM, 1.0, false, Align::C);
        c.rect_outline(p.x, p.y, p.w, p.h, FRAME, 1.0, 1.0);

        let a = ((f.saturating_sub(PLOT_START)) as f32 / 15.0).min(1.0);
        // 理論線 σ/√N
        let line: Vec<Pt> = (0..=30).map(|k| {
            let n = 10f64.powf(3.0 * k as f64 / 30.0);
            (xp(n), yp(self.sigma / n.sqrt()))
        }).collect();
        c.polyline(&line, IONIZE, a, 2.0);
        // 実測の点を1つずつ
        let shown = (f.saturating_sub(PLOT_START + 15) / 8) as usize;
        for &(n, sd) in self.plot.iter().take(shown) {
            c.circle(xp(n as f64), yp(sd), 5.0, ELEC, 1.0);
            c.ring(xp(n as f64), yp(sd), 5.0, WHITE, 0.8, 1.2);
        }
        // 凡例
        let ly = p.y + p.h + 64.0;
        c.circle(p.x + 6.0, ly - 5.0, 5.0, ELEC, a);
        c.text(&format!("実測 ({BATCHES}回の平均の SD)"), p.x + 18.0, ly, 13, FG, a, false, Align::L);
        c.line(p.x, ly + 17.0, p.x + 14.0, ly + 17.0, IONIZE, a, 2.0);
        c.text(&format!("理論 σ/√N  (σ = {:.1})", self.sigma), p.x + 18.0, ly + 22.0, 13, FG, a, false, Align::L);
        c.text("傾き −1/2 の直線になる", p.x, ly + 50.0, 14, IONIZE, a, true, Align::L);
    }
}

impl Scene for CltScene {
    fn title(&self) -> &'static str {
        "平均の分布"
    }
    fn frames(&self) -> u32 {
        540
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        c.clear();
        widgets::header(c, "平均利得の分布", "N 個の光子で平均した利得は正規分布に近づくか");
        let (hx, hw, rh, gap, y0) = (230.0, 640.0, 100.0, 22.0, 92.0);
        for (i, r) in self.rows.iter().enumerate() {
            let start = ROW_START + i as u32 * ROW_STEP;
            let shown = (f.saturating_sub(start) as usize * r.means.len()) / ROW_FILL as usize;
            let curve_a = (f.saturating_sub(start + ROW_FILL) as f32 / 15.0).min(1.0);
            if f >= start {
                self.draw_row(c, r, Rect::new(hx, y0 + i as f32 * (rh + gap), hw, rh), shown, curve_a);
            }
        }
        // 共通の横軸
        let ya = y0 + 4.0 * (rh + gap) - gap + 16.0;
        for k in (0..=60).step_by(10) {
            c.text(&k.to_string(), hx + k as f32 / X_MAX as f32 * hw, ya, 12, DIM, 1.0, false, Align::C);
        }
        c.text("N 個で平均した利得 →", hx + hw, ya + 18.0, 12, DIM, 1.0, false, Align::R);
        c.line(hx + 10.0, 78.0, hx + 24.0, 78.0, IONIZE, 1.0, 2.0);
        c.text("正規分布 N(μ, σ²/N)", hx + 30.0, 83.0, 12, DIM, 1.0, false, Align::L);
        c.text(&format!("破線 = 全体の平均 μ = {:.1}", self.mu), hx + 210.0, 83.0, 12, DIM, 1.0, false, Align::L);
        if f >= PLOT_START {
            self.draw_plot(c, f);
        }

        let err = self.sigma / 1000f64.sqrt();
        let pct = if self.mu > 0.0 { err / self.mu * 100.0 } else { 0.0 };
        widgets::captions(
            c,
            f,
            &[
                (0, 130, format!("N 個の光子の平均利得を {BATCHES} 回ずつ求め、N を変えて分布を比べる")),
                (130, 260, "N=1 は裾の長いゆがんだ分布。N を増やすと釣り鐘形 (正規分布) に近づく = 中心極限定理".to_string()),
                (260, 400, "平均のばらつき (標準偏差) は σ/√N で小さくなる: N を 100 倍 → 1/10".to_string()),
                (400, 540, format!("N=1000 なら平均 {:.1} の誤差は ±{err:.1} (約 {pct:.0}%)。光子が多いほど M を精度よく測れる", self.mu)),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_integrates_to_one_inside_range() {
        let v = [1.0, 2.5, 3.0, 59.9, 70.0];
        let (d, over) = density(&v, 0.5);
        assert_eq!(over, 1);
        let area: f64 = d.iter().sum::<f64>() * 0.5;
        assert!((area - 0.8).abs() < 1e-9);
    }

    #[test]
    fn normal_pdf_peak() {
        assert!((normal_pdf(0.0, 0.0, 1.0) - 0.398_942_28).abs() < 1e-6);
        assert_eq!(normal_pdf(0.0, 0.0, 0.0), 0.0);
    }
}
