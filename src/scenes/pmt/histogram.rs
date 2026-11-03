//! PMT シーン: 光電子 1 個あたりの増倍率の分布 (1,000 回の事前計算 + 動画中に追加)

use crate::physics::pmt::{precompute_pmt_gains, sample_gain, PmtSpec};
use crate::physics::stats::gain_stats;
use crate::physics::Rng;
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::{Align, Canvas};
use crate::scenes::{edge_fade, Scene};

pub const PRECOMPUTED: usize = 1000;
const SEED: u64 = 0x9417;
/// ビン幅と本数 (0〜48000 を 30 本 + 最後に「48000 以上」)
const BIN: u32 = 1600;
const NBIN: usize = 30;
/// 1 フレームに追加する回数
const LIVE_PER_FRAME: usize = 2;
const GROW: u32 = 40;
const FRAMES: u32 = 420;

fn bin_of(m: u32) -> usize {
    ((m / BIN) as usize).min(NBIN)
}

pub struct PmtHistogramScene {
    spec: PmtSpec,
    pre: Vec<u32>,
    live: Vec<u32>,
    rng: Rng,
    /// 縦軸の最大 (最後まで追加しても収まるように最初に決める)
    y_max: f32,
    /// 追加されたビンを光らせる (ビン, 追加フレーム)
    flashes: Vec<(usize, u32)>,
}

impl PmtHistogramScene {
    pub fn new() -> Self {
        let spec = PmtSpec::standard();
        let pre = precompute_pmt_gains(&spec, PRECOMPUTED, SEED);
        let mut counts = [0u32; NBIN + 1];
        for &m in &pre {
            counts[bin_of(m)] += 1;
        }
        let peak = counts.iter().copied().max().unwrap_or(1) as f32;
        let total = PRECOMPUTED as f32 + (FRAMES - GROW) as f32 * LIVE_PER_FRAME as f32;
        let y_max = ((peak * total / PRECOMPUTED as f32 * 1.2) / 20.0).ceil() * 20.0;
        PmtHistogramScene { spec, pre, live: Vec::new(), rng: Rng::seeded(SEED ^ 0xFF), y_max, flashes: Vec::new() }
    }
}

impl Scene for PmtHistogramScene {
    fn title(&self) -> &'static str {
        "PMT ヒストグラム"
    }
    fn frames(&self) -> u32 {
        FRAMES
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        if f >= GROW {
            for _ in 0..LIVE_PER_FRAME {
                let m = sample_gain(&self.spec, &mut self.rng);
                self.live.push(m);
                self.flashes.push((bin_of(m), f));
            }
        }
        self.flashes.retain(|&(_, t0)| f.saturating_sub(t0) < 12);
        let mut cnt_pre = [0u32; NBIN + 1];
        let mut cnt_live = [0u32; NBIN + 1];
        for &m in &self.pre {
            cnt_pre[bin_of(m)] += 1;
        }
        for &m in &self.live {
            cnt_live[bin_of(m)] += 1;
        }
        let all: Vec<u32> = self.pre.iter().chain(&self.live).copied().collect();
        let Some(st) = gain_stats(&all) else { return };
        let zeros = all.iter().filter(|&&m| m == 0).count();

        c.clear();
        widgets::header(c, "PMT の増倍率の分布", "光電子 1 個あたり、アノードに届いた電子数");

        // ヒストグラム
        let h = Rect::new(110.0, 110.0, 820.0, 410.0);
        let grow = 1.0 - (1.0 - (f as f32 / GROW as f32).min(1.0)).powi(3);
        c.rect(h.x, h.y, h.w, h.h, PANEL, 1.0);
        let steps = 5;
        for k in 0..=steps {
            let v = self.y_max * k as f32 / steps as f32;
            let yy = h.y + h.h - v / self.y_max * h.h;
            c.line(h.x, yy, h.x + h.w, yy, GRID, 0.6, 1.0);
            c.text(&format!("{v:.0}"), h.x - 8.0, yy + 5.0, 13, DIM, 1.0, false, Align::R);
        }
        c.text("回数", h.x - 8.0, h.y - 12.0, 14, DIM, 1.0, false, Align::R);
        let bw = h.w / (NBIN + 1) as f32;
        let base = h.y + h.h;
        for i in 0..=NBIN {
            let x = h.x + i as f32 * bw;
            let hp = (cnt_pre[i] as f32 * grow / self.y_max * h.h).min(h.h);
            let hl = cnt_live[i] as f32 / self.y_max * h.h;
            c.rect(x + 1.0, base - hp, bw - 2.0, hp, if i == NBIN { HIST_OVER } else { HIST_BAR }, 0.95);
            if hl > 0.0 {
                let top = (base - hp - hl).max(h.y);
                c.rect(x + 1.0, top, bw - 2.0, (base - hp) - top, HOLE, 1.0);
            }
        }
        for &(b, t0) in &self.flashes {
            let k = 1.0 - f.saturating_sub(t0) as f32 / 12.0;
            let x = h.x + b as f32 * bw;
            let hb = (cnt_pre.get(b).copied().unwrap_or(0) + cnt_live.get(b).copied().unwrap_or(0)) as f32 / self.y_max * h.h;
            let top = (base - hb).max(h.y);
            c.rect(x - 1.0, top - 3.0, bw + 2.0, base - top + 3.0, WHITE, 0.18 * k);
        }
        c.rect_outline(h.x, h.y, h.w, h.h, FRAME, 1.0, 1.0);
        for k in (0..=40000).step_by(8000) {
            c.text(&k.to_string(), h.x + k as f32 / BIN as f32 * bw, base + 20.0, 13, DIM, 1.0, false, Align::C);
        }
        c.text("48000〜", h.x + NBIN as f32 * bw + bw / 2.0, base + 20.0, 12, DIM, 1.0, false, Align::C);
        c.text("増倍率 (光電子 1 個あたりの電子数)", h.x + h.w / 2.0, base + 44.0, 15, FG, 1.0, false, Align::C);
        if f >= GROW {
            let a = ((f - GROW) as f32 / 15.0).min(1.0);
            let lines = [(st.mean as f32, FG, format!("平均 {:.0}", st.mean), 0.0), (st.median as f32, FIELD, format!("中央値 {}", st.median), 22.0)];
            for (v, col, label, dy) in lines {
                let x = h.x + v / BIN as f32 * bw;
                c.dashed_vline(x, h.y, base, col, 0.9 * a, 1.5);
                c.text(&label, x + 6.0, h.y + 20.0 + dy, 14, col, a, true, Align::L);
            }
        }

        // 右: 統計
        let sx = 985.0;
        widgets::value_rows(
            c,
            sx,
            140.0,
            250.0,
            34.0,
            16,
            &[
                ("事前計算", self.pre.len().to_string(), HIST_BAR),
                ("+ 動画中に追加", self.live.len().to_string(), HOLE),
                ("平均 〈M〉", format!("{:.0}", st.mean), FG),
                (&format!("設計値 {} の {} 乗", self.spec.delta, self.spec.stages), format!("{:.0}", self.spec.mean_gain()), DIM),
                ("中央値", st.median.to_string(), FIELD),
                ("ゆらぎ σ", format!("{:.0}", st.sd), FG),
                ("相対ゆらぎ σ/〈M〉", format!("{:.2}", st.sd / st.mean), FG),
            ],
        );
        c.line(sx, 372.0, sx + 250.0, 372.0, GRID, 1.0, 1.0);
        c.text("過剰雑音係数 F", sx, 400.0, 16, DIM, 1.0, false, Align::L);
        c.text(&format!("{:.2}", st.excess_noise), sx + 250.0, 446.0, 38, IONIZE, 1.0, true, Align::R);
        c.text(&format!("理論値 1 + 1/4 + 1/16 + … = {:.3}", self.spec.excess_noise_theory()), sx, 476.0, 13, DIM, 1.0, false, Align::L);
        c.text(&format!("増倍率 0 (ほぼ D1 で二次電子 0 個): {zeros} 回"), sx, 504.0, 13, DIM, 1.0, false, Align::L);
        c.rect(sx, 540.0, 14.0, 14.0, HIST_BAR, 1.0);
        c.text(&format!("事前計算 {PRECOMPUTED} 回"), sx + 20.0, 552.0, 13, FG, 1.0, false, Align::L);
        c.rect(sx, 563.0, 14.0, 14.0, HOLE, 1.0);
        c.text("動画中に追加", sx + 20.0, 575.0, 13, FG, 1.0, false, Align::L);

        let share = self.spec.first_stage_share() * 100.0;
        widgets::captions(
            c,
            f,
            &[
                (0, 140, format!("光電子 1 個あたりの増倍率を {PRECOMPUTED} 回計算した分布 (各段の二次電子数はポアソン分布)")),
                (140, 280, format!("平均 ≈ {:.0} (設計値 {:.0})。中央値 {} も平均に近く、平均の周りにまとまった分布", st.mean, self.spec.mean_gain(), st.median)),
                (280, FRAMES, format!("F = 1 + 1/4 + 1/16 + … ≈ 1.33。その約 {share:.0}% は 1 段目のゆらぎ (1/4) から来る")),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}
