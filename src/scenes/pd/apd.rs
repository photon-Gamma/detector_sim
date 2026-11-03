//! シーン 2: APD のなだれ増倍 (1光子のスロー → 連続光)

use crate::scenes::{edge_fade, Scene};
use crate::physics::{DeviceSpec, Rng, Sim};
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::Canvas;

/// 1光子デモで M が平均 (≈23) に近い値になるように選んだ乱数の種
pub const DEMO_SEED: u64 = 5u64.wrapping_mul(0x9E37_79B9_7F4A_7C15);
/// 光子を入れるフレーム
const DEMO_START: u32 = 50;
/// 連続光を当て始めるフレーム
const LIGHT_ON: u32 = 420;
const PHOTON_RATE: f32 = 0.05;

/// 再生速度: 吸収〜ドリフトは 0.4 倍、なだれ増倍は 0.25 倍のスロー
pub fn playback_dt(f: u32) -> f32 {
    match f {
        150..=289 => 0.25,
        50..=149 => 0.4,
        _ => 1.0,
    }
}

pub struct ApdScene {
    sim: Sim,
    rng: Rng,
}

impl ApdScene {
    pub fn new() -> Self {
        ApdScene { sim: Sim::new(DeviceSpec::apd(), DEMO_SEED), rng: Rng::seeded(0xA9D) }
    }
}

impl Scene for ApdScene {
    fn title(&self) -> &'static str {
        "APD"
    }
    fn frames(&self) -> u32 {
        720
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        let slow = (DEMO_START..290).contains(&f);
        if f == DEMO_START {
            self.sim.emit(0.45, Some(0.25));
        }
        if f == LIGHT_ON {
            self.sim.reset_counts();
        }
        if f >= LIGHT_ON && self.rng.chance(PHOTON_RATE) {
            self.sim.emit_random();
        }
        self.sim.step(playback_dt(f));

        c.clear();
        widgets::header(c, "アバランシェフォトダイオード (APD)", "内部でなだれ増倍 (M ≫ 1)");
        let dev = Rect::new(150.0, 150.0, 380.0, 400.0);
        widgets::device(c, &self.sim, dev, f, true);
        widgets::field_profile(c, &self.sim, dev);
        widgets::current_graph(c, &self.sim, Rect::new(790.0, 130.0, 450.0, 220.0), 2.2, "外部回路に流れる光電流 i(t)   [縦軸は Si PD の10倍]", ELEC);
        widgets::count_panel(c, &self.sim, 830.0, 410.0, true);
        widgets::legend(c, 800.0, 590.0);
        if slow {
            widgets::badge(c, "スロー再生  1光子だけ", PHOTON);
        } else if f >= LIGHT_ON {
            widgets::badge(c, "連続光を照射", PHOTON);
        }
        widgets::captions(
            c,
            f,
            &[
                (0, 50, "リーチスルー型 APD：光を吸収する π 層と、電界の強い増倍層を分けた構造"),
                (50, 100, "① 光子が π 層で吸収され、電子−正孔対が1組生まれる"),
                (100, 160, "② 電子は電界に引かれて、増倍層 (p−n⁺ 接合付近) へ向かう"),
                (160, 220, "③ 強い電界で加速された電子が原子に衝突 → 新しい電子−正孔対を叩き出す (衝突電離)"),
                (220, 420, "④ 生まれた電子もまた衝突電離を起こし、連鎖的に増える = なだれ増倍"),
                (420, 570, "連続光でも、1つ1つの光子がなだれを起こして大きな電流パルスになる"),
                (570, 720, "増倍のたびに個数がばらつく → M は平均値。ばらつきは「過剰雑音」になる"),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 種や物理パラメータを変えたとき、1光子デモが極端な値にならないかの確認
    #[test]
    fn demo_photon_gives_representative_gain() {
        let mut s = Sim::new(DeviceSpec::apd(), DEMO_SEED);
        // 正孔が全部抜けて M が確定するまで (連続光は入れない)
        for f in 0..720 {
            if f == DEMO_START {
                s.emit(0.45, Some(0.25));
            }
            s.step(playback_dt(f));
        }
        // 平均 ≈ 23 なので、デモはその近く (極端に小さい/大きい種を選ばない)
        assert!(matches!(s.gains(), [m] if (15..=35).contains(m)), "{:?}", s.gains());
    }
}
