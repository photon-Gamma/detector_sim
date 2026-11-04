//! 画面サイズと配色。

/// 出力解像度 (レイアウトはこのサイズ前提)
pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;
pub const FPS: u32 = 30;

pub type Rgb = (u8, u8, u8);

pub const BG: Rgb = (14, 19, 32);
pub const PANEL: Rgb = (22, 29, 46);
pub const GRID: Rgb = (60, 70, 92);
pub const FRAME: Rgb = (70, 82, 108);
pub const FG: Rgb = (230, 236, 245);
pub const DIM: Rgb = (140, 152, 172);
pub const WHITE: Rgb = (255, 255, 255);

pub const ELEC: Rgb = (64, 170, 255);
pub const HOLE: Rgb = (255, 122, 69);
pub const PHOTON: Rgb = (236, 120, 250);
pub const IONIZE: Rgb = (255, 225, 77);
pub const GOLD: Rgb = (212, 175, 90);
pub const FIELD: Rgb = (120, 220, 160);
pub const HIST_BAR: Rgb = (90, 140, 210);
pub const HIST_OVER: Rgb = (150, 110, 190);
