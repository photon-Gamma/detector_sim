//! シーン 0: タイトル

use crate::scenes::{edge_fade, Scene};
use crate::render::raster::Pt;
use crate::render::theme::*;
use crate::render::{Align, Canvas};

pub struct Title;

impl Scene for Title {
    fn title(&self) -> &'static str {
        "タイトル"
    }
    fn frames(&self) -> u32 {
        120
    }
    fn render(&mut self, c: &mut Canvas, f: u32) {
        c.clear();
        let cx = WIDTH as f32 / 2.0;
        let a = (f as f32 / 25.0).min(1.0);
        c.text("光検出器のしくみ", cx, 300.0, 64, FG, a, true, Align::C);
        let b = ((f as f32 - 20.0) / 25.0).clamp(0.0, 1.0);
        c.text("Si PIN フォトダイオード  と  アバランシェフォトダイオード (APD)", cx, 370.0, 26, DIM, b, false, Align::C);
        // 降ってくる光子
        for k in 0..5 {
            let x = 400.0 + k as f32 * 120.0;
            let t = ((f as f32 * 4.0 + k as f32 * 37.0) % 160.0) / 160.0;
            let y = 430.0 + t * 120.0;
            let pts: Vec<Pt> = (0..=16).map(|i| (x + 4.0 * (i as f32 * 0.9 + f as f32 * 0.5).sin(), y - 40.0 + i as f32 * 2.5)).collect();
            c.polyline(&pts, PHOTON, b * (1.0 - t), 2.5);
        }
        edge_fade(c, f, self.frames());
    }
}
