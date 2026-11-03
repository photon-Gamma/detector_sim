//! シーン 4: 1光子ごとの利得 M のヒストグラム
//! (1,000 回の事前計算 + 動画中にライブで追加)

use crate::scenes::{edge_fade, Scene};
use crate::physics::stats::{gain_stats, precompute_gains};
use crate::physics::{mix_seed, DeviceSpec, Rng, Sim};
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::{Align, Canvas};

/// 事前計算する試行回数
pub const PRECOMPUTED: usize = 1000;
const PRECOMPUTE_SEED: u64 = 0xC0FFEE;
/// ビン幅と数 (0〜150 を 30 本 + 最後に「150 以上」)
const BIN: u32 = 5;
const NBIN: usize = 30;
/// 縦軸の最大 (回数)
const Y_MAX: f32 = 300.0;
/// ライブ計算の速さ (1フレームで何ステップ進めるか) と同時に飛ばす光子の上限
const LIVE_SPEED: usize = 4;
const LIVE_MAX_IN_FLIGHT: usize = 3;
/// バーが伸びるアニメーションの長さ
const GROW: u32 = 40;

fn bin_of(m: u32) -> usize {
    ((m / BIN) as usize).min(NBIN)
}

pub struct HistogramScene {
    /// 10 万回で求めた (〈M〉, σ, F)
    reference: (f64, f64, f64),
    pre: Vec<u32>,
    live: Sim,
    rng: Rng,
    /// live.gains() のうち取り込んだ数
    seen: usize,
    /// 追加されたビンを光らせる (ビン, M, 追加フレーム)
    flashes: Vec<(usize, u32, u32)>,
}

impl HistogramScene {
    pub fn new() -> Self {
        HistogramScene {
            pre: precompute_gains(PRECOMPUTED, PRECOMPUTE_SEED),
            reference: super::compare::reference_stats(),
            live: Sim::new(DeviceSpec::apd(), mix_seed(0x5EED)),
            rng: Rng::seeded(0x415),
            seen: 0,
            flashes: Vec::new(),
        }
    }

    fn advance_live(&mut self, f: u32) {
        for _ in 0..LIVE_SPEED {
            if f >= GROW && self.live.photons().is_empty() && self.live.photons_in_progress() < LIVE_MAX_IN_FLIGHT && self.rng.chance(0.08) {
                self.live.emit_random();
            }
            self.live.step(1.0);
        }
        let new = self.live.gains().get(self.seen..).unwrap_or(&[]).to_vec();
        for m in new {
            self.flashes.push((bin_of(m), m, f));
            self.seen += 1;
        }
        self.flashes.retain(|&(_, _, t0)| f.saturating_sub(t0) < 20);
    }
}

impl Scene for HistogramScene {
    fn title(&self) -> &'static str {
        "利得ヒストグラム"
    }
    fn frames(&self) -> u32 {
        420
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        self.advance_live(f);
        let mut cnt_pre = [0u32; NBIN + 1];
        let mut cnt_live = [0u32; NBIN + 1];
        for &m in &self.pre {
            cnt_pre[bin_of(m)] += 1;
        }
        for &m in self.live.gains() {
            cnt_live[bin_of(m)] += 1;
        }
        let all: Vec<u32> = self.pre.iter().chain(self.live.gains()).copied().collect();
        let Some(st) = gain_stats(&all) else { return };

        c.clear();
        widgets::header(c, "APD の利得 M の分布", "1光子ずつ入れたときの増倍数");

        // 左: ライブで回している APD
        widgets::device(c, &self.live, Rect::new(60.0, 150.0, 180.0, 300.0), f, false);
        c.text("4倍速・1光子ごとに", 150.0, 495.0, 15, DIM, 1.0, false, Align::C);
        c.text("M を数えて追加", 150.0, 517.0, 15, DIM, 1.0, false, Align::C);

        // 中央: ヒストグラム
        let h = Rect::new(330.0, 120.0, 650.0, 400.0);
        let grow = 1.0 - (1.0 - (f as f32 / GROW as f32).min(1.0)).powi(3);
        c.rect(h.x, h.y, h.w, h.h, PANEL, 1.0);
        for k in 0..=6 {
            let v = k as f32 * 50.0;
            let yy = h.y + h.h - v / Y_MAX * h.h;
            c.line(h.x, yy, h.x + h.w, yy, GRID, 0.6, 1.0);
            c.text(&format!("{v}"), h.x - 8.0, yy + 5.0, 13, DIM, 1.0, false, Align::R);
        }
        c.text("回数", h.x - 8.0, h.y - 12.0, 14, DIM, 1.0, false, Align::R);
        let bw = h.w / (NBIN + 1) as f32;
        let base = h.y + h.h;
        for i in 0..=NBIN {
            let x = h.x + i as f32 * bw;
            let hp = (cnt_pre[i] as f32 * grow / Y_MAX * h.h).min(h.h);
            let hl = cnt_live[i] as f32 / Y_MAX * h.h;
            c.rect(x + 1.0, base - hp, bw - 2.0, hp, if i == NBIN { HIST_OVER } else { HIST_BAR }, 0.95);
            if hl > 0.0 {
                let y_top = (base - hp - hl).max(h.y);
                c.rect(x + 1.0, y_top, bw - 2.0, (base - hp) - y_top, IONIZE, 1.0);
            }
        }
        // 追加されたビンを光らせる
        for &(b, m, t0) in &self.flashes {
            let age = f.saturating_sub(t0) as f32;
            let k = 1.0 - age / 20.0;
            let x = h.x + b as f32 * bw;
            let hb = (cnt_pre.get(b).copied().unwrap_or(0) + cnt_live.get(b).copied().unwrap_or(0)) as f32 / Y_MAX * h.h;
            let y_top = (base - hb).max(h.y);
            c.rect(x - 2.0, y_top - 4.0, bw + 4.0, base - y_top + 4.0, IONIZE, 0.25 * k);
            c.text(&format!("+1 (M={m})"), x + bw / 2.0, y_top - 10.0 - age * 1.5, 14, IONIZE, k, true, Align::C);
        }
        c.rect_outline(h.x, h.y, h.w, h.h, FRAME, 1.0, 1.0);
        for k in (0..=125).step_by(25) {
            c.text(&k.to_string(), h.x + k as f32 / BIN as f32 * bw, base + 20.0, 13, DIM, 1.0, false, Align::C);
        }
        c.text("150〜", h.x + NBIN as f32 * bw + bw / 2.0, base + 20.0, 12, DIM, 1.0, false, Align::C);
        c.text("利得 M (1光子あたりの電子数)", h.x + h.w / 2.0, base + 44.0, 15, FG, 1.0, false, Align::C);

        // 平均と中央値の縦線
        if f >= GROW {
            let a = ((f - GROW) as f32 / 15.0).min(1.0);
            let lines = [(st.mean as f32, FG, format!("平均 {:.1}", st.mean), 0.0), (st.median as f32, FIELD, format!("中央値 {}", st.median), 22.0)];
            for (v, col, label, dy) in lines {
                let x = h.x + v / BIN as f32 * bw;
                c.dashed_vline(x, h.y, base, col, 0.9 * a, 1.5);
                c.text(&label, x + 6.0, h.y + 20.0 + dy, 14, col, a, true, Align::L);
            }
        }

        // 右: 統計
        let sx = 1020.0;
        widgets::value_rows(
            c,
            sx,
            160.0,
            220.0,
            36.0,
            17,
            &[
                ("事前計算", self.pre.len().to_string(), HIST_BAR),
                ("+ ライブ", self.live.gains().len().to_string(), IONIZE),
                ("平均 〈M〉", format!("{:.1}", st.mean), FG),
                ("中央値", st.median.to_string(), FIELD),
                ("ゆらぎ σ", format!("{:.1}", st.sd), FG),
                ("最大", st.max.to_string(), FG),
            ],
        );
        c.line(sx, 358.0, sx + 220.0, 358.0, GRID, 1.0, 1.0);
        c.text("過剰雑音係数", sx, 400.0, 17, DIM, 1.0, false, Align::L);
        c.text("F = 〈M²〉/〈M〉²", sx, 424.0, 15, DIM, 1.0, false, Align::L);
        c.text(&format!("{:.2}", st.excess_noise), sx + 220.0, 470.0, 38, IONIZE, 1.0, true, Align::R);
        c.text(&format!("参考: 10 万回では F = {:.2}", self.reference.2), sx, 500.0, 14, FG, 1.0, false, Align::L);
        c.text("Si PIN PD は F = 1", sx, 522.0, 13, DIM, 1.0, false, Align::L);
        c.rect(sx, 545.0, 14.0, 14.0, HIST_BAR, 1.0);
        c.text(&format!("事前計算 {PRECOMPUTED}回"), sx + 20.0, 557.0, 13, FG, 1.0, false, Align::L);
        c.rect(sx, 568.0, 14.0, 14.0, IONIZE, 1.0);
        c.text("この動画中に追加", sx + 20.0, 580.0, 13, FG, 1.0, false, Align::L);

        widgets::captions(
            c,
            f,
            &[
                (0, 140, format!("1光子あたりの増倍数 M を {PRECOMPUTED} 回シミュレーションした分布")),
                (140, 280, format!("平均 {:.0} でも中央値は {}。M が 100 を超える光子もある (長い裾)", st.mean, st.median)),
                (280, 350, format!("この {} 回では F ≈ {:.1}。裾が長いので、回数が少ないと推定が大きくぶれる", st.n, st.excess_noise)),
                (350, 420, format!("10 万回では F ≈ {:.2} → 増倍後の雑音は理想 (F=1) の √F ≈ {:.2} 倍", self.reference.2, self.reference.2.sqrt())),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}
