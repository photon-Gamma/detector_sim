//! シーン 1: Si PIN フォトダイオード (1光子のスロー → 連続光)

use crate::scenes::{edge_fade, Scene};
use crate::physics::{DeviceSpec, Rng, Sim};
use crate::render::theme::*;
use crate::render::widgets::{self, Rect};
use crate::render::Canvas;

/// 1光子デモのスロー区間
const SLOW: std::ops::Range<u32> = 45..300;
/// 連続光を当て始めるフレーム
const LIGHT_ON: u32 = 300;
/// 連続光での1フレームあたりの光子到来確率
const PHOTON_RATE: f32 = 0.12;

pub struct SiPinScene {
    sim: Sim,
    rng: Rng,
}

impl SiPinScene {
    pub fn new() -> Self {
        SiPinScene { sim: Sim::new(DeviceSpec::si_pin(), 0x9E37_79B9_7F4A_7C15), rng: Rng::seeded(0x51) }
    }
}

impl Scene for SiPinScene {
    fn title(&self) -> &'static str {
        "Si PIN PD"
    }
    fn frames(&self) -> u32 {
        540
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        let slow = SLOW.contains(&f);
        if f == SLOW.start {
            self.sim.emit(0.45, Some(0.50));
        }
        if f == LIGHT_ON {
            self.sim.reset_counts();
        }
        if f >= LIGHT_ON && self.rng.chance(PHOTON_RATE) {
            self.sim.emit_random();
        }
        self.sim.step(if slow { 0.4 } else { 1.0 });

        c.clear();
        widgets::header(c, "Si PIN フォトダイオード", "増倍なし (M = 1)");
        let dev = Rect::new(150.0, 150.0, 380.0, 400.0);
        widgets::device(c, &self.sim, dev, f, true);
        widgets::field_profile(c, &self.sim, dev);
        widgets::current_graph(c, &self.sim, Rect::new(790.0, 130.0, 450.0, 220.0), 0.22, "外部回路に流れる光電流 i(t)", ELEC);
        widgets::count_panel(c, &self.sim, 830.0, 420.0, false);
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
                (0, 45, "p⁺ / i / n⁺ 構造に逆バイアス → i 層全体が空乏層になり、一様な電界がかかる"),
                (45, 100, "① 光子が表面の窓から入り、空乏層 (i 層) で吸収される"),
                (100, 150, "② 吸収されると 電子−正孔対 が1組生まれる"),
                (150, 300, "③ 電界で 電子は n側へ、正孔は p側へドリフト → 動いている間、外部に電流が流れる"),
                (300, 420, "光が当たり続けると、光子の数 (光の強さ) に比例した光電流になる"),
                (420, 540, "増倍がないので 1光子 → 電子1個分の電荷  (利得 M = 1)"),
            ],
        );
        edge_fade(c, f, self.frames());
    }
}
