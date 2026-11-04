//! PMT シーン: PMT (7 段) と HPD (打ち込み 200 + APD 70) の増倍ゆらぎと過剰雑音係数 F の比較

use crate::physics::hpd::{self, APD_DESIGN_GAIN, EB_GAIN};
use crate::physics::pmt::{precompute_pmt_gains, PmtSpec};
use crate::physics::stats::{gain_stats, GainStats};
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::{Align, Canvas};
use crate::scenes::{edge_fade, Scene};

pub const TRIALS: usize = 20_000;
const X_MAX: f64 = 2.5;
const BIN: f64 = 0.04;
const FRAMES: u32 = 540;
const PMT_COL: Rgb = ELEC;
const HPD_COL: Rgb = FIELD;

struct Dist {
    stats: GainStats,
    density: Vec<f64>,
    over: f64,
}

fn summarize(v: &[u32]) -> Option<Dist> {
    let stats = gain_stats(v)?;
    let nb = (X_MAX / BIN).round() as usize;
    let mut h = vec![0.0; nb];
    let mut over = 0usize;
    for &m in v {
        let i = (f64::from(m) / stats.mean / BIN).floor() as usize;
        match h.get_mut(i) {
            Some(c) => *c += 1.0,
            None => over += 1,
        }
    }
    let n = v.len() as f64;
    Some(Dist { stats, density: h.into_iter().map(|c| c / (n * BIN)).collect(), over: over as f64 / n })
}

pub struct PmtVsHpdScene {
    spec: PmtSpec,
    pmt: Option<Dist>,
    hpd: Option<Dist>,
    pmt_theory: f64,
    hpd_theory: f64,
    /// 2 段目の APD 単体の F
    apd_f: f64,
}

impl PmtVsHpdScene {
    pub fn new() -> Self {
        let spec = PmtSpec::standard();
        eprintln!("  (PMT {TRIALS} 回と HPD {TRIALS} 回を事前計算しています…)");
        let pmt = summarize(&precompute_pmt_gains(&spec, TRIALS, 0x5EED_9417));
        let hpd = summarize(&hpd::precompute_hpd_gains(TRIALS, 0x05EE_D4BD));
        let apd_f = hpd::apd70_stats().map_or(f64::NAN, |s| s.excess_noise);
        PmtVsHpdScene {
            spec,
            pmt,
            hpd,
            pmt_theory: spec.excess_noise_theory(),
            hpd_theory: hpd::cascade_excess_noise(hpd::eb_excess_noise(), EB_GAIN, apd_f),
            apd_f,
        }
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

fn opt(d: &Option<Dist>, f: impl Fn(&Dist) -> String) -> String {
    d.as_ref().map_or("—".to_string(), f)
}

impl Scene for PmtVsHpdScene {
    fn title(&self) -> &'static str {
        "PMT と HPD の比較"
    }
    fn frames(&self) -> u32 {
        FRAMES
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        c.clear();
        let pmt_mean = self.pmt.as_ref().map_or(0.0, |d| d.stats.mean);
        let hpd_mean = self.hpd.as_ref().map_or(0.0, |d| d.stats.mean);
        widgets::header(c, "増倍のゆらぎ: PMT と HPD", &format!("平均増倍率 PMT 約 {:.1} 万、HPD 約 {:.1} 万", pmt_mean / 1e4, hpd_mean / 1e4));

        // 分布
        let r = Rect::new(90.0, 130.0, 740.0, 380.0);
        let y_max = [&self.pmt, &self.hpd].iter().filter_map(|d| d.as_ref()).flat_map(|d| d.density.iter().copied()).fold(0.0f64, f64::max).max(1e-9) * 1.08;
        c.rect(r.x, r.y, r.w, r.h, PANEL, 1.0);
        for k in 0..=5 {
            let x = r.x + k as f32 * r.w / 5.0;
            c.line(x, r.y, x, r.y + r.h, GRID, 0.5, 1.0);
            c.text(&format!("{}", k as f32 * 0.5), x, r.y + r.h + 20.0, 13, DIM, 1.0, false, Align::C);
        }
        c.text("M / 〈M〉 (平均増倍率で割った値)", r.x + r.w / 2.0, r.y + r.h + 44.0, 15, FG, 1.0, false, Align::C);
        c.text("確率密度", r.x - 10.0, r.y - 12.0, 13, DIM, 1.0, false, Align::R);
        let ease = |t: f32| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
        if let Some(d) = &self.pmt {
            Self::draw_dist(c, d, r, y_max, ease(f as f32 / 45.0), PMT_COL);
        }
        if let Some(d) = &self.hpd {
            Self::draw_dist(c, d, r, y_max, ease((f as f32 - 30.0) / 45.0), HPD_COL);
        }
        let x1 = r.x + r.w / X_MAX as f32;
        c.dashed_vline(x1, r.y, r.y + r.h, FG, 0.7, 1.2);
        c.text("平均", x1 + 6.0, r.y + 18.0, 13, FG, 0.9, false, Align::L);
        c.rect_outline(r.x, r.y, r.w, r.h, FRAME, 1.0, 1.0);
        c.rect(r.x + r.w - 280.0, r.y + 14.0, 16.0, 12.0, PMT_COL, 0.9);
        c.text(&format!("PMT ({} 段、各段 {})", self.spec.stages, self.spec.delta), r.x + r.w - 258.0, r.y + 25.0, 14, FG, 1.0, false, Align::L);
        c.rect(r.x + r.w - 280.0, r.y + 38.0, 16.0, 12.0, HPD_COL, 0.9);
        c.text(&format!("HPD (打ち込み {EB_GAIN:.0} × APD {APD_DESIGN_GAIN:.0})"), r.x + r.w - 258.0, r.y + 49.0, 14, FG, 1.0, false, Align::L);
        if f >= 75 {
            let over = format!("{X_MAX} を超える割合: PMT {}、HPD {}", opt(&self.pmt, |d| format!("{:.1}%", d.over * 100.0)), opt(&self.hpd, |d| format!("{:.1}%", d.over * 100.0)));
            c.text(&over, r.x + r.w - 10.0, r.y + 76.0, 12, DIM, 1.0, false, Align::R);
        }

        // 比較表
        let (tx, c1, c2) = (870.0, 1120.0, 1235.0);
        let a = ((f as f32 - 80.0) / 20.0).clamp(0.0, 1.0);
        c.text("PMT", c1, 150.0, 16, PMT_COL, a, true, Align::R);
        c.text("HPD", c2, 150.0, 16, HPD_COL, a, true, Align::R);
        c.line(tx, 160.0, c2, 160.0, GRID, a, 1.0);
        let rows: [(&str, String, String); 7] = [
            ("平均増倍率 〈M〉", opt(&self.pmt, |d| format!("{:.0}", d.stats.mean)), opt(&self.hpd, |d| format!("{:.0}", d.stats.mean))),
            ("1 段目のゲイン G1", format!("{}", self.spec.delta), format!("{EB_GAIN:.0}")),
            ("1 段目のゆらぎ 1/√G1", format!("{:.2}", 1.0 / self.spec.delta.sqrt()), format!("{:.3}", 1.0 / EB_GAIN.sqrt())),
            ("相対ゆらぎ σ/〈M〉", opt(&self.pmt, |d| format!("{:.3}", d.stats.sd / d.stats.mean)), opt(&self.hpd, |d| format!("{:.3}", d.stats.sd / d.stats.mean))),
            ("過剰雑音係数 F", opt(&self.pmt, |d| format!("{:.3}", d.stats.excess_noise)), opt(&self.hpd, |d| format!("{:.3}", d.stats.excess_noise))),
            ("F の理論値", format!("{:.3}", self.pmt_theory), format!("{:.3}", self.hpd_theory)),
            ("雑音の倍率 √F", opt(&self.pmt, |d| format!("{:.3}", d.stats.excess_noise.sqrt())), opt(&self.hpd, |d| format!("{:.3}", d.stats.excess_noise.sqrt()))),
        ];
        for (i, (name, p, q)) in rows.iter().enumerate() {
            let y = 192.0 + i as f32 * 38.0;
            let k = ((f as f32 - 90.0 - i as f32 * 8.0) / 15.0).clamp(0.0, 1.0);
            let hl = i == 4;
            c.text(name, tx, y, 15, if hl { FG } else { DIM }, k, hl, Align::L);
            c.text(p, c1, y, if hl { 20 } else { 17 }, PMT_COL, k, true, Align::R);
            c.text(q, c2, y, if hl { 20 } else { 17 }, HPD_COL, k, true, Align::R);
        }
        let k = ((f as f32 - 160.0) / 15.0).clamp(0.0, 1.0);
        let last = self.spec.delta.powi(self.spec.stages as i32 - 1);
        c.text(&format!("PMT: F = 1 + 1/{} + … + 1/{last:.0}", self.spec.delta), tx, 470.0, 12, DIM, k, false, Align::L);
        c.text(&format!("HPD: F = F1 + (F2 − 1)/G1 = {:.3} + ({:.2} − 1)/{EB_GAIN:.0}", hpd::eb_excess_noise(), self.apd_f), tx, 490.0, 12, DIM, k, false, Align::L);
        c.text("F1: 打ち込み (ポアソン)、F2: APD 単体", tx, 510.0, 12, DIM, k, false, Align::L);
        c.text(&format!("シミュレーション: 各 {TRIALS} 回"), tx, 530.0, 12, DIM, k, false, Align::L);

        let (fp, fh) = (self.pmt.as_ref().map_or(0.0, |d| d.stats.excess_noise), self.hpd.as_ref().map_or(0.0, |d| d.stats.excess_noise));
        widgets::captions(
            c,
            f,
            &[
                (0, 130, "平均増倍率はどちらも 1 万倍台。横軸を M/〈M〉 にそろえて分布の形を比べる".to_string()),
                (130, 260, "HPD の分布は細く、平均のまわりに集まる。PMT は広がりが大きい".to_string()),
                (260, 400, format!("過剰雑音係数 F: PMT {fp:.2}、HPD {fh:.3} → 雑音は理想の √F 倍 ({:.2} 倍 と {:.2} 倍)", fp.sqrt(), fh.sqrt())),
                (400, 470, format!("2 段目の APD 単体は F ≈ {:.1} と大きいが、その余分 (F − 1) は 1/{EB_GAIN:.0} に薄まる", self.apd_f)),
                (470, FRAMES, "F = F1 + (F2 − 1)/G1: 1 段目のゲイン G1 が大きいほど、後の段のゆらぎは効かない".to_string()),
            ],
        );
        widgets::source_note(c, "F の式: Shockley & Pierce (1938)、Teich ら, IEEE JQE 22, 1184 (1986)。打ち込みはポアソン分布と仮定");
        edge_fade(c, f, self.frames());
    }
}
