//! シーン 3: 同じ光を Si PIN PD と APD に入れて、出力と利得のゆらぎを比べる

use crate::scenes::{edge_fade, Scene};
use crate::physics::{mix_seed, DeviceSpec, Rng, Sim};
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::physics::stats::{gain_stats, precompute_gains};
use crate::render::{Align, Canvas};

const PHOTON_RATE: f32 = 0.07;
/// 2つのグラフで共通の縦軸の最大値
const Y_MAX: f32 = 3.0;

/// 参考値を求める光子の数
const REF_TRIALS: usize = 100_000;

pub struct CompareScene {
    si: Sim,
    apd: Sim,
    rng: Rng,
    /// 多数の光子での APD の (〈M〉, σ, F)
    reference: (f64, f64, f64),
}

/// 多数の光子で求めた APD の利得の統計 (動画の数個だけの値と比べるため)
pub fn reference_stats() -> (f64, f64, f64) {
    gain_stats(&precompute_gains(REF_TRIALS, 0xC0FFEE)).map_or((0.0, 0.0, 0.0), |s| (s.mean, s.sd, s.excess_noise))
}

impl CompareScene {
    pub fn new() -> Self {
        CompareScene {
            si: Sim::new(DeviceSpec::si_pin(), mix_seed(11)),
            apd: Sim::new(DeviceSpec::apd(), mix_seed(0xABC_DEF1_2345)),
            rng: Rng::seeded(0x123_4567),
            reference: reference_stats(),
        }
    }
}

impl Scene for CompareScene {
    fn title(&self) -> &'static str {
        "比較"
    }
    fn frames(&self) -> u32 {
        360
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        // 同じ横位置・同じタイミングの光子を両方に入れる
        if self.rng.chance(PHOTON_RATE) {
            let x = self.rng.range(0.2, 0.8);
            self.si.emit(x, None);
            self.apd.emit(x, None);
        }
        self.si.step(1.0);
        self.apd.step(1.0);

        c.clear();
        widgets::header(c, "同じ光を入れたときの出力", "縦軸は同じ目盛");
        widgets::device(c, &self.si, Rect::new(70.0, 140.0, 160.0, 170.0), f, false);
        widgets::device(c, &self.apd, Rect::new(70.0, 395.0, 160.0, 170.0), f, false);
        widgets::current_graph(c, &self.si, Rect::new(270.0, 130.0, 620.0, 180.0), Y_MAX, "Si PIN PD", ELEC);
        widgets::current_graph(c, &self.apd, Rect::new(270.0, 385.0, 620.0, 180.0), Y_MAX, "APD", IONIZE);
        widgets::gain_panel(c, &self.si, 935.0, 150.0, ELEC);
        widgets::gain_panel(c, &self.apd, 935.0, 405.0, IONIZE);
        c.text("光子が少ないうちは値がぶれる。10 万光子では", 935.0, 585.0, 12, DIM, 1.0, false, Align::L);
        c.text(&format!("〈M〉 = {:.1}、σ = {:.1}、F = {:.2}", self.reference.0, self.reference.1, self.reference.2), 935.0, 603.0, 12, DIM, 1.0, false, Align::L);
        widgets::captions(
            c,
            f,
            &[
                (0, 120, "同じ光量でも、APD は内部増倍で約 〈M〉 倍の信号 → 微弱光の検出に強い"),
                (120, 240, "ただし光子ごとの M はばらばら。Si PD は全部 M=1 なので σ=0, F=1"),
                (240, 360, "ゆらぎの大きさは過剰雑音係数 F = 〈M²〉/〈M〉² で表す (F が大きいほど雑音増)"),
            ],
        );
        widgets::source_note(c, "過剰雑音係数 F の定義: McIntyre, IEEE Trans. ED 13, 164 (1966)");
        edge_fade(c, f, self.frames());
    }
}
