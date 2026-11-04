//! PMT 動画のタイトル

use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::{Align, Canvas};
use crate::scenes::{edge_fade, Scene};

pub struct PmtTitle;

impl Scene for PmtTitle {
    fn title(&self) -> &'static str {
        "PMT タイトル"
    }
    fn frames(&self) -> u32 {
        120
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        c.clear();
        let cx = WIDTH as f32 / 2.0;
        let a = (f as f32 / 25.0).min(1.0);
        c.text("PMT と HPD のしくみ", cx, 270.0, 60, FG, a, true, Align::C);
        let b = ((f as f32 - 20.0) / 25.0).clamp(0.0, 1.0);
        c.text("光電子増倍管 (PMT) とハイブリッド光検出器 (HPD)", cx, 335.0, 28, FG, b, false, Align::C);
        c.text("増え方のちがいと、増倍のゆらぎ (過剰雑音係数 F) を比べる", cx, 380.0, 22, DIM, b, false, Align::C);
        // 段ごとに約 4 倍に増える電子のイメージ (1 → 4 → 16 個の点)
        for s in 0..3 {
            let x = 470.0 + s as f32 * 170.0;
            let side = 1usize << s; // 1, 2, 4 個四方 = 1, 4, 16 個
            let k = ((f as f32 - 30.0 - s as f32 * 14.0) / 15.0).clamp(0.0, 1.0);
            for i in 0..side * side {
                let (gx, gy) = ((i % side) as f32, (i / side) as f32);
                let off = (side - 1) as f32 / 2.0;
                c.circle(x + (gx - off) * 13.0, 475.0 + (gy - off) * 13.0, 4.5, ELEC, k);
            }
            if s < 2 {
                let pts: Vec<Pt> = vec![(x + 40.0, 475.0), (x + 125.0, 475.0)];
                c.polyline(&pts, DIM, 0.5 * k, 1.5);
                c.text("× 4", x + 82.0, 465.0, 14, DIM, k, false, Align::C);
            }
        }
        edge_fade(c, f, self.frames());
    }
}
