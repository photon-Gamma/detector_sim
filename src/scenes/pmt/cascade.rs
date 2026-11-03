//! PMT シーン: 光電子 1 個が 7 段のダイノードで増える様子 (スロー → 早送りでくり返し)

use crate::physics::pmt::{Node, PmtSim, PmtSpec};
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::{widgets, Align, Canvas};
use crate::scenes::{edge_fade, Scene};
use std::f32::consts::PI;

/// スロー再生の 1 回で、増倍率が平均 (16384) に近くなるように選んだ乱数の種 (tests で確認)
pub const DEMO_SEED: u64 = 21;
/// 光子が光電面に届くフレーム
const PHOTON_HIT: u32 = 40;
/// スロー再生の速さ
const SLOW_DT: f32 = 0.9;
/// くり返しを始めるフレームと速さ
const REPEAT_START: u32 = 440;
const FRAMES: u32 = 650;
const FAST_DT: f32 = 6.0;
/// 画面に描く電子の最大数 (それ以上は間引いて描く。数値は全部数える)
const MAX_DRAWN: usize = 700;

const CATHODE: Pt = (108.0, 255.0);
const ANODE: Pt = (1150.0, 255.0);
const Y_LOW: f32 = 335.0;
const Y_HIGH: f32 = 175.0;

pub struct CascadeScene {
    sim: PmtSim,
    glow: Vec<f32>,
}

impl CascadeScene {
    pub fn new() -> Self {
        let spec = PmtSpec::standard();
        CascadeScene { glow: vec![0.0; spec.stages + 2], sim: PmtSim::new(spec, DEMO_SEED) }
    }
}

/// ノードの画面上の位置 (0 = 光電面, 1..=6 = ダイノード, 7 = アノード)
fn node_pos(n: Node, stages: usize) -> Pt {
    if n == 0 {
        CATHODE
    } else if n > stages {
        ANODE
    } else {
        let i = (n - 1) as f32;
        (230.0 + i * 125.0, if (n - 1).is_multiple_of(2) { Y_LOW } else { Y_HIGH })
    }
}

/// ノード a → a+1 の軌道上の点。lane はビームの中の横位置
fn path_point(a: Node, t: f32, lane: f32, stages: usize) -> Pt {
    let p0 = node_pos(a, stages);
    let p1 = node_pos(a + 1, stages);
    let t = t.clamp(0.0, 1.0);
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let ctrl = ((p0.0 + p1.0) / 2.0 + nx * 28.0, (p0.1 + p1.1) / 2.0 + ny * 28.0);
    let u = 1.0 - t;
    let bx = u * u * p0.0 + 2.0 * u * t * ctrl.0 + t * t * p1.0;
    let by = u * u * p0.1 + 2.0 * u * t * ctrl.1 + t * t * p1.1;
    let spread = 16.0 * (PI * t).sin() * lane;
    (bx + nx * spread, by + ny * spread)
}

fn draw_tube(c: &mut Canvas, sim: &PmtSim, glow: &[f32]) {
    let stages = sim.spec().stages;
    c.rect(55.0, 105.0, 1170.0, 300.0, PANEL, 0.7);
    c.rect_outline(55.0, 105.0, 1170.0, 300.0, FRAME, 1.0, 2.0);
    c.text("真空のガラス管", 70.0, 128.0, 13, DIM, 1.0, false, Align::L);
    c.text("後ろの段ほど高い電圧 →", 1210.0, 128.0, 13, DIM, 1.0, false, Align::R);
    // 光電面
    let cath: Vec<Pt> = (0..=12).map(|k| {
        let s = k as f32 / 12.0 - 0.5;
        (96.0 + 40.0 * s * s, 255.0 + s * 190.0)
    }).collect();
    c.polyline(&cath, (190, 150, 95), 1.0, 7.0);
    if let Some(&g) = glow.first() {
        c.polyline(&cath, IONIZE, g * 0.8, 14.0);
    }
    c.text("光電面", 96.0, 382.0, 14, FG, 1.0, true, Align::C);
    // ダイノード
    for n in 1..=stages {
        let (x, y) = node_pos(n, stages);
        let low = (n - 1).is_multiple_of(2);
        let s = if low { 1.0 } else { -1.0 };
        let cup: Vec<Pt> = [(-34.0, -14.0), (-18.0, 2.0), (0.0, 7.0), (18.0, 2.0), (34.0, -14.0)].iter().map(|&(dx, dy)| (x + dx, y + s * dy)).collect();
        let g = glow.get(n).copied().unwrap_or(0.0);
        if g > 0.01 {
            c.polyline(&cup, IONIZE, g * 0.8, 16.0);
        }
        c.polyline(&cup, (165, 172, 190), 1.0, 7.0);
        let ly = if low { y + 40.0 } else { y - 30.0 };
        c.text(&format!("D{n}"), x, ly, 15, FG, 1.0, true, Align::C);
        let a = sim.arrivals().get(n).copied().unwrap_or(0);
        if a > 0 {
            c.text(&format!("{a} 個"), x, ly + if low { 20.0 } else { -20.0 }, 14, ELEC, 1.0, true, Align::C);
        }
    }
    // アノード
    c.rect(ANODE.0 - 6.0, ANODE.1 - 45.0, 14.0, 90.0, GOLD, 1.0);
    if let Some(&g) = glow.last() {
        c.rect(ANODE.0 - 12.0, ANODE.1 - 51.0, 26.0, 102.0, IONIZE, g * 0.4);
    }
    c.text("アノード", ANODE.0, ANODE.1 + 72.0, 14, FG, 1.0, true, Align::C);
    let a = sim.arrivals().get(sim.anode()).copied().unwrap_or(0);
    if a > 0 {
        c.text(&format!("{a} 個"), ANODE.0, ANODE.1 + 92.0, 14, ELEC, 1.0, true, Align::C);
    }
}

fn draw_electrons(c: &mut Canvas, sim: &PmtSim) {
    let stages = sim.spec().stages;
    let es = sim.electrons();
    let stride = es.len().div_ceil(MAX_DRAWN).max(1);
    for e in es.iter().step_by(stride) {
        let p = path_point(e.from, e.t, e.lane, stages);
        c.circle(p.0, p.1, 5.5, ELEC, 0.25);
        c.circle(p.0, p.1, 3.5, ELEC, 1.0);
    }
    if stride > 1 {
        c.text(&format!("※ 電子は {stride} 個に 1 個だけ表示"), 1210.0, 395.0, 12, DIM, 1.0, false, Align::R);
    }
}

/// 光電面に向かう光子 (左から水平に)
fn draw_photon(c: &mut Canvas, f: u32) {
    if f >= PHOTON_HIT {
        return;
    }
    let x_tip = 10.0 + (CATHODE.0 - 20.0) * f as f32 / PHOTON_HIT as f32;
    let pts: Vec<Pt> = (0..=20).map(|k| {
        let t = k as f32 / 20.0;
        (x_tip - 50.0 * (1.0 - t), CATHODE.1 + 5.0 * (t * 14.0 + f as f32 * 0.6).sin())
    }).collect();
    c.polyline(&pts, PHOTON, 0.35, 7.0);
    c.polyline(&pts, PHOTON, 1.0, 2.5);
    c.polygon(&[(x_tip + 9.0, CATHODE.1), (x_tip - 2.0, CATHODE.1 - 6.0), (x_tip - 2.0, CATHODE.1 + 6.0)], PHOTON, 1.0);
}

/// 各段に届いた電子数の棒グラフ (対数目盛) と設計値
fn draw_stage_chart(c: &mut Canvas, sim: &PmtSim) {
    let (x0, base, h) = (110.0, 588.0, 112.0);
    // 1 個の棒も見えるように、目盛の下端は 1 より少し下 (10^-0.3) にする
    let (lo, decades) = (-0.3f32, 5.3f32);
    let py = |v: f64| base - ((v.max(1.0).log10() as f32 - lo) / decades) * h;
    c.text("各段に届いた電子の数 (対数目盛)", 60.0, 452.0, 15, FG, 1.0, true, Align::L);
    for (k, label) in ["1", "10", "100", "1000", "1万", "10万"].iter().enumerate() {
        let y = base - (k as f32 - lo) / decades * h;
        c.line(x0, y, x0 + 510.0, y, GRID, 0.6, 1.0);
        c.text(label, x0 - 8.0, y + 4.0, 11, DIM, 1.0, false, Align::R);
    }
    let stages = sim.spec().stages;
    let bw = 510.0 / (stages + 2) as f32;
    for n in 0..=stages + 1 {
        let x = x0 + n as f32 * bw;
        let v = sim.arrivals().get(n).copied().unwrap_or(0);
        if v > 0 {
            let top = py(v as f64);
            c.rect(x + 8.0, top, bw - 16.0, (base - top).max(2.0), ELEC, 0.9);
        }
        // 設計値: 光電面と D1 に届くのは 1 個、D(k) には 4^(k-1) 個、アノードには 4^6 個
        let design = sim.spec().delta.powi(n.saturating_sub(1) as i32);
        let yd = py(design);
        c.line(x + 4.0, yd, x + bw - 4.0, yd, FG, 0.9, 2.0);
        let label = match n {
            0 => "光電面".to_string(),
            n if n > stages => "アノード".to_string(),
            n => format!("D{n}"),
        };
        c.text(&label, x + bw / 2.0, base + 16.0, 11, DIM, 1.0, false, Align::C);
    }
    c.line(470.0, 452.0 - 5.0, 490.0, 452.0 - 5.0, FG, 0.9, 2.0);
    c.text("設計値 (4 倍ずつ)", 496.0, 452.0, 12, DIM, 1.0, false, Align::L);
}

fn draw_result_panel(c: &mut Canvas, sim: &PmtSim) {
    let x = 720.0;
    let a = sim.arrivals().get(sim.anode()).copied().unwrap_or(0);
    c.text("今回の増倍率 (アノードに届いた電子数)", x, 452.0, 15, FG, 1.0, true, Align::L);
    let shown = if a > 0 { a.to_string() } else { "—".to_string() };
    c.text(&shown, x + 200.0, 505.0, 40, ELEC, 1.0, true, Align::R);
    let spec = sim.spec();
    c.text(&format!("設計値の平均: {} の {} 乗 = {:.0}", spec.delta, spec.stages, spec.mean_gain()), x, 538.0, 14, DIM, 1.0, false, Align::L);
    let done = sim.finished();
    if done.len() > 1 {
        let recent: Vec<String> = done.iter().rev().take(6).rev().map(|v| v.to_string()).collect();
        c.text("これまでの結果:", x, 572.0, 14, DIM, 1.0, false, Align::L);
        c.text(&recent.join(",  "), x + 110.0, 572.0, 15, FG, 1.0, true, Align::L);
    }
}

/// 設計値の列 "1 → 4 → 16 → …"
fn design_sequence(spec: &PmtSpec) -> String {
    (0..=spec.stages).map(|k| format!("{:.0}", spec.delta.powi(k as i32))).collect::<Vec<_>>().join(" → ")
}

/// 100 の位に丸める (8192 → "8200")
fn round_sig(v: f64) -> String {
    format!("{:.0}", (v / 100.0).round() * 100.0)
}

impl Scene for CascadeScene {
    fn title(&self) -> &'static str {
        "PMT の増倍"
    }
    fn frames(&self) -> u32 {
        FRAMES
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        if f == PHOTON_HIT {
            self.sim.emit_photoelectron();
            if let Some(g) = self.glow.first_mut() {
                *g = 1.0;
            }
        }
        let fast = f >= REPEAT_START;
        if fast && !self.sim.busy() {
            self.sim.emit_photoelectron();
        }
        self.sim.step(if fast { FAST_DT } else { SLOW_DT });
        for (g, &h) in self.glow.iter_mut().zip(self.sim.hits()) {
            *g *= 0.88;
            if h > 0 {
                *g = (*g + 0.25 + 0.1 * (h as f32).log2()).min(1.0);
            }
        }

        c.clear();
        let spec = *self.sim.spec();
        widgets::header(c, "光電子増倍管 (PMT) の増倍", &format!("ダイノード {} 段、各段の設計ゲイン {}", spec.stages, spec.delta));
        draw_tube(c, &self.sim, &self.glow);
        draw_photon(c, f);
        draw_electrons(c, &self.sim);
        draw_stage_chart(c, &self.sim);
        draw_result_panel(c, &self.sim);
        if fast {
            widgets::badge(c, "早送り  光電子 1 個ずつくり返す", ELEC);
        } else if f >= PHOTON_HIT {
            widgets::badge(c, "スロー再生  光電子 1 個", ELEC);
        }
        widgets::captions(
            c,
            f,
            &[
                (0, 45, format!("PMT は光電面・ダイノード {} 段・アノードを真空管に収めた構造", spec.stages)),
                (45, 85, "① 光子が光電面に当たり、光電子が 1 個飛び出す (光電効果)".to_string()),
                (85, 165, format!("② 電界で加速された電子が D1 に衝突 → 二次電子が平均 {} 個出る", spec.delta)),
                (165, 330, format!("③ 段ごとに約 {} 倍: {}", spec.delta, design_sequence(&spec))),
                (330, REPEAT_START, format!("④ アノードに集まった電子が出力パルス。平均の増倍率は {} の {} 乗 = {:.0}", spec.delta, spec.stages, spec.mean_gain())),
                (REPEAT_START, 545, "同じ光電子 1 個でも、増倍率は毎回違う (二次電子の数がばらつくため)".to_string()),
                (545, FRAMES, format!(
                    "特に 1 段目が効く: D1 から 2 個出れば期待値は約 {}、6 個なら約 {}",
                    round_sig(2.0 * spec.mean_gain() / spec.delta),
                    round_sig(6.0 * spec.mean_gain() / spec.delta)
                )),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// スロー再生の 1 回が、平均から極端に外れていないか (種を変えたときの確認)
    #[test]
    fn demo_cascade_is_representative_and_finishes_in_time() {
        let mut s = PmtSim::new(PmtSpec::standard(), DEMO_SEED);
        for f in 0..REPEAT_START {
            if f == PHOTON_HIT {
                s.emit_photoelectron();
            }
            s.step(SLOW_DT);
        }
        assert!(!s.busy(), "くり返しの前に 1 回目が終わっていない");
        let g = s.finished().first().copied().unwrap_or(0);
        assert!((14_500..=18_500).contains(&g), "demo gain {g}");
    }
}
