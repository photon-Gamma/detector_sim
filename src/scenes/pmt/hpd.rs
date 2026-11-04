//! HPD シーン: 光電子を APD に打ち込んで約 200 倍、さらに APD のなだれ増倍で約 70 倍
//!
//! 数値 (打ち込みでできた対の数・合計の増倍率) は physics::hpd の計算結果そのもの。
//! 粒子の動きは見せ方のための演出で、APD 内の電子は一部だけ描く。

use crate::physics::hpd::{apd70_pool, apd70_stats, sample_event, HpdEvent, APD_DESIGN_GAIN, EB_GAIN};
use crate::physics::Rng;
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::{widgets, Align, Canvas};
use crate::scenes::{edge_fade, Scene};

/// 最初の 1 回 (スロー) が平均に近くなるように選んだ種 (tests で確認)
pub const DEMO_SEED: u64 = 54;
const FRAMES: u32 = 600;
/// 1 回分の演出の長さ (速さ 1 のとき)
const CYCLE: f32 = 270.0;
const REPEAT_START: u32 = 300;
const FAST: f32 = 6.0;
const MAX_DRAWN: usize = 600;

// 画面上の位置
const CATHODE_X: f32 = 108.0;
const MID_Y: f32 = 255.0;
const APD_X0: f32 = 860.0; // 打ち込み面
const DRIFT_END: f32 = 990.0; // ここから増倍層
const MULT_END: f32 = 1040.0; // ここから n⁺
const APD_X1: f32 = 1080.0;
const APD_Y0: f32 = 165.0;
const APD_Y1: f32 = 345.0;

/// 1 回分の演出
struct Run {
    event: HpdEvent,
    start: u32,
    speed: f32,
}

pub struct HpdScene {
    pool: &'static [u32],
    rng: Rng,
    runs: Vec<Run>,
    apd_mean: f64,
}

impl HpdScene {
    pub fn new() -> Self {
        let pool = apd70_pool();
        let mut rng = Rng::seeded(DEMO_SEED);
        let first = sample_event(pool, &mut rng);
        HpdScene {
            pool,
            rng,
            runs: vec![Run { event: first, start: 40, speed: 1.0 }],
            apd_mean: apd70_stats().map_or(70.0, |s| s.mean),
        }
    }

    /// 演出の進み具合 (0 = 光電子が出た瞬間、CYCLE = 終わり)
    fn phase(run: &Run, f: u32) -> f32 {
        (f.saturating_sub(run.start)) as f32 * run.speed
    }
}

/// 疑似乱数 (描画の散らばり用。同じ i なら毎フレーム同じ値)
fn hash01(i: u64) -> f32 {
    let mut z = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z ^= z >> 29;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 32;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

fn draw_tube(c: &mut Canvas) {
    c.rect(55.0, 105.0, 1170.0, 300.0, PANEL, 0.7);
    c.rect_outline(55.0, 105.0, 1170.0, 300.0, FRAME, 1.0, 2.0);
    c.text("真空のガラス管", 70.0, 128.0, 13, DIM, 1.0, false, Align::L);
    // 光電面
    let cath: Vec<Pt> = (0..=12).map(|k| {
        let s = k as f32 / 12.0 - 0.5;
        (96.0 + 40.0 * s * s, MID_Y + s * 190.0)
    }).collect();
    c.polyline(&cath, (190, 150, 95), 1.0, 7.0);
    c.text("光電面", 96.0, 382.0, 14, FG, 1.0, true, Align::C);
    // 加速の電界 (光電面 → APD)
    for k in 0..5 {
        let y = 175.0 + k as f32 * 40.0;
        c.line(200.0, y, 780.0, y, DIM, 0.25, 1.0);
        c.polygon(&[(792.0, y), (780.0, y - 5.0), (780.0, y + 5.0)], DIM, 0.35);
    }
    c.text("高電圧で光電子を加速 →", 490.0, 150.0, 15, FG, 0.9, false, Align::C);
    c.text("光電面: −HV", 160.0, 395.0, 13, GOLD, 1.0, false, Align::L);
    // APD
    let layers = [
        (APD_X0, APD_X0 + 12.0, (120, 60, 72)),
        (APD_X0 + 12.0, DRIFT_END, (44, 52, 74)),
        (DRIFT_END, MULT_END, (88, 62, 70)),
        (MULT_END, APD_X1, (50, 78, 120)),
    ];
    for (x0, x1, col) in layers {
        c.rect(x0, APD_Y0, x1 - x0, APD_Y1 - APD_Y0, col, 1.0);
    }
    c.rect(DRIFT_END, APD_Y0, MULT_END - DRIFT_END, APD_Y1 - APD_Y0, IONIZE, 0.15);
    c.rect_outline(DRIFT_END, APD_Y0, MULT_END - DRIFT_END, APD_Y1 - APD_Y0, IONIZE, 0.6, 1.2);
    c.text("APD", (APD_X0 + APD_X1) / 2.0, APD_Y0 - 12.0, 16, FG, 1.0, true, Align::C);
    c.text("打ち込み面", APD_X0 - 4.0, APD_Y1 + 20.0, 12, DIM, 1.0, false, Align::C);
    c.text("なだれ増倍層", (DRIFT_END + MULT_END) / 2.0, APD_Y1 + 38.0, 12, IONIZE, 1.0, false, Align::C);
    c.text("出力 →", 1160.0, MID_Y + 5.0, 14, FG, 1.0, false, Align::C);
}

/// 1 回分の粒子を描く
fn draw_run(c: &mut Canvas, run: &Run, p: f32) {
    let n = run.event.eb_pairs as usize;
    // 0〜40: 光子 (最初の回だけ)。0〜50: 光電子が加速しながら飛ぶ
    if p < 50.0 {
        let t = (p / 50.0).clamp(0.0, 1.0);
        let x = CATHODE_X + (APD_X0 - CATHODE_X) * t * t;
        c.circle(x, MID_Y, 7.0, ELEC, 0.3);
        c.circle(x, MID_Y, 4.5, ELEC, 1.0);
        return;
    }
    // 50: 打ち込み。光るリング
    if p < 70.0 {
        let k = (p - 50.0) / 20.0;
        c.ring(APD_X0 + 10.0, MID_Y, 10.0 + 60.0 * k, IONIZE, 1.0 - k, 3.0);
    }
    // 50〜110: 打ち込みでできた対。正孔は打ち込み面へ、電子は増倍層へ
    if p < 160.0 {
        for i in 0..n.min(MAX_DRAWN) {
            let (r1, r2) = (hash01(i as u64 * 2 + run.start as u64 * 7919), hash01(i as u64 * 2 + 1 + run.start as u64 * 7919));
            let y = MID_Y + (r1 - 0.5) * 150.0;
            let x_born = APD_X0 + 14.0 + r2 * 22.0;
            let t = ((p - 50.0) / 60.0).clamp(0.0, 1.0);
            let ex = x_born + (DRIFT_END + 4.0 - x_born) * t;
            c.circle(ex, y, 3.0, ELEC, 1.0);
            if p < 70.0 {
                let hx = x_born - (x_born - APD_X0 - 2.0) * ((p - 50.0) / 20.0);
                c.circle(hx, y, 3.0, HOLE, 0.9);
            }
        }
    }
    // 110〜230: なだれ増倍。電子が増え、n⁺ 側へ流れて出ていく
    if (110.0..240.0).contains(&p) {
        let grow = ((p - 110.0) / 80.0).clamp(0.0, 1.0);
        let shown = ((n as f32) + (MAX_DRAWN as f32 - n as f32).max(0.0) * grow) as usize;
        let leave = ((p - 190.0) / 50.0).clamp(0.0, 1.0);
        for i in 0..shown.min(MAX_DRAWN) {
            let (r1, r2) = (hash01(i as u64 * 3 + 11), hash01(i as u64 * 3 + 12));
            let y = MID_Y + (r1 - 0.5) * 160.0;
            let x = DRIFT_END + 4.0 + r2 * (APD_X1 - DRIFT_END - 8.0) + leave * 120.0 * (0.5 + r2);
            if x < APD_X1 + 60.0 {
                c.circle(x, y, 2.6, ELEC, 1.0 - leave * 0.6);
            }
        }
    }
}

/// その時点での電子の数 (表示用)。打ち込み前 1、打ち込み後 n、増倍中は n から合計まで増える
fn count_at(run: &Run, p: f32) -> (u64, u64) {
    let n = u64::from(run.event.eb_pairs);
    if p < 50.0 {
        return (1, 1);
    }
    if p < 110.0 {
        return (n, n);
    }
    let g = ((p - 110.0) / 80.0).clamp(0.0, 1.0) as f64;
    let total = run.event.total as f64;
    // 対数的に増やす (なだれは掛け算で増えるので)
    let now = (n.max(1) as f64) * (total / n.max(1) as f64).powf(g);
    (n, now.round() as u64)
}

fn draw_chart(c: &mut Canvas, run: &Run, p: f32) {
    let (x0, base, h) = (110.0, 588.0, 112.0);
    let (lo, decades) = (-0.3f32, 5.3f32);
    let py = |v: f64| base - ((v.max(1.0).log10() as f32 - lo) / decades) * h;
    c.text("電子の数 (対数目盛)", 60.0, 452.0, 15, FG, 1.0, true, Align::L);
    for (k, label) in ["1", "10", "100", "1000", "1万", "10万"].iter().enumerate() {
        let y = base - (k as f32 - lo) / decades * h;
        c.line(x0, y, x0 + 510.0, y, GRID, 0.6, 1.0);
        c.text(label, x0 - 8.0, y + 4.0, 11, DIM, 1.0, false, Align::R);
    }
    let (n, now) = count_at(run, p);
    let stages: [(&str, f64, Option<u64>); 3] = [
        ("光電子", 1.0, Some(1)),
        ("打ち込み後", EB_GAIN, (p >= 50.0).then_some(n)),
        ("APD 増倍後", EB_GAIN * APD_DESIGN_GAIN, (p >= 110.0).then_some(now)),
    ];
    let bw = 510.0 / 3.0;
    for (i, (label, design, value)) in stages.iter().enumerate() {
        let x = x0 + i as f32 * bw;
        if let Some(v) = value {
            let top = py(*v as f64);
            c.rect(x + 30.0, top, bw - 60.0, (base - top).max(2.0), ELEC, 0.9);
            c.text(&v.to_string(), x + bw / 2.0, top - 6.0, 13, ELEC, 1.0, true, Align::C);
        }
        let yd = py(*design);
        c.line(x + 22.0, yd, x + bw - 22.0, yd, FG, 0.9, 2.0);
        c.text(label, x + bw / 2.0, base + 16.0, 12, DIM, 1.0, false, Align::C);
    }
    c.line(470.0, 447.0, 490.0, 447.0, FG, 0.9, 2.0);
    c.text("設計値の平均", 496.0, 452.0, 12, DIM, 1.0, false, Align::L);
}

impl Scene for HpdScene {
    fn title(&self) -> &'static str {
        "HPD の増倍"
    }
    fn frames(&self) -> u32 {
        FRAMES
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        // くり返し: 前の回が終わったら次の回を始める
        if f >= REPEAT_START {
            let done = self.runs.last().is_none_or(|r| Self::phase(r, f) >= CYCLE);
            if done {
                let ev = sample_event(self.pool, &mut self.rng);
                self.runs.push(Run { event: ev, start: f, speed: FAST });
            }
        }
        let Some(run) = self.runs.last() else { return };
        let p = Self::phase(run, f);

        c.clear();
        widgets::header(c, "ハイブリッド光検出器 (HPD) の増倍", &format!("電子打ち込み (平均 {EB_GAIN:.0}) + APD (平均 {APD_DESIGN_GAIN:.0})"));
        draw_tube(c);
        // 光子 (最初の回だけ、光電面に向かう)
        if f < 40 {
            let x_tip = 10.0 + (CATHODE_X - 20.0) * f as f32 / 40.0;
            let pts: Vec<Pt> = (0..=20).map(|k| {
                let t = k as f32 / 20.0;
                (x_tip - 50.0 * (1.0 - t), MID_Y + 5.0 * (t * 14.0 + f as f32 * 0.6).sin())
            }).collect();
            c.polyline(&pts, PHOTON, 1.0, 2.5);
        } else {
            draw_run(c, run, p);
        }
        draw_chart(c, run, if f < 40 { -1.0 } else { p });

        // 右: 今回の結果
        let x = 720.0;
        c.text("今回の増倍率 (出力の電子数)", x, 452.0, 15, FG, 1.0, true, Align::L);
        let finished = p >= 240.0;
        let shown = if finished { run.event.total.to_string() } else { "—".to_string() };
        c.text(&shown, x + 200.0, 505.0, 40, ELEC, 1.0, true, Align::R);
        c.text(&format!("設計値の平均: {EB_GAIN:.0} × {APD_DESIGN_GAIN:.0} = {:.0}", EB_GAIN * APD_DESIGN_GAIN), x, 538.0, 14, DIM, 1.0, false, Align::L);
        c.text(&format!("(シミュレーションの APD の平均は {:.1})", self.apd_mean), x + 330.0, 538.0, 12, DIM, 1.0, false, Align::L);
        let done: Vec<String> = self.runs.iter().filter(|r| Self::phase(r, f) >= 240.0).map(|r| r.event.total.to_string()).collect();
        if done.len() > 1 {
            let recent: Vec<String> = done.iter().rev().take(5).rev().cloned().collect();
            c.text("これまでの結果:", x, 572.0, 14, DIM, 1.0, false, Align::L);
            c.text(&recent.join(", "), x + 110.0, 572.0, 15, FG, 1.0, true, Align::L);
        }
        if f >= REPEAT_START {
            widgets::badge(c, "早送り  光電子 1 個ずつくり返す", ELEC);
        } else if f >= 40 {
            widgets::badge(c, "スロー再生  光電子 1 個", ELEC);
        }
        if (50.0..240.0).contains(&p) {
            c.text("※ APD の中の電子は一部だけ表示", 1210.0, 395.0, 12, DIM, 1.0, false, Align::R);
        }

        let rel1 = 1.0 / EB_GAIN.sqrt() * 100.0;
        widgets::captions(
            c,
            f,
            &[
                (0, 40, "HPD は光電面と APD を真空管に収めた検出器".to_string()),
                (40, 90, "① 光電面から出た光電子を、高電圧で加速して APD に打ち込む".to_string()),
                (90, 150, format!("② 打ち込まれた電子がエネルギーを失いながら、電子−正孔対を約 {EB_GAIN:.0} 組作る")),
                (150, 240, format!("③ できた電子それぞれが APD のなだれ増倍で約 {APD_DESIGN_GAIN:.0} 倍 → 合計 約 {:.0} 倍", EB_GAIN * APD_DESIGN_GAIN)),
                (240, REPEAT_START, format!("1 段目で一気に約 {EB_GAIN:.0} 倍になるのが、PMT (1 段目 約 4 倍) との大きなちがい")),
                (REPEAT_START, 450, "くり返すと、合計の増倍率のばらつきは PMT よりずっと小さい".to_string()),
                (450, FRAMES, format!("1 段目の相対ゆらぎ: HPD は 1/√{EB_GAIN:.0} ≈ {rel1:.0}%、PMT は 1/√4 = 50%")),
            ],
        );
        widgets::source_note(c, "実機の例: 打ち込み約 1500 倍 (−8 kV)、APD 約 80 倍 (浜松ホトニクス HPD TPMH1361E)。動画は指定の 200 × 70");
        edge_fade(c, f, self.frames());
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// スロー再生の 1 回が平均に近いか (種や APD の設定を変えたときの確認)
    #[test]
    fn demo_event_is_representative() {
        let mean = apd70_stats().map_or(70.0, |s| s.mean) * EB_GAIN;
        let e = sample_event(apd70_pool(), &mut Rng::seeded(DEMO_SEED));
        assert!((190..=210).contains(&e.eb_pairs), "{e:?}");
        assert!((e.total as f64 / mean - 1.0).abs() < 0.05, "{e:?} mean {mean}");
    }
}
