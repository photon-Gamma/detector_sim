//! 描画 (物理とは独立)。
//!
//! - `raster`:  アンチエイリアス付きの塗りつぶし (被覆率の計算)
//! - `font`:    TrueType フォントの読み込み
//! - `canvas`:  1フレーム分の画像と、図形・文字の描画 API
//! - `theme`:   画面サイズ・色
//! - `widgets`: デバイス断面図・グラフ・字幕など画面部品

pub mod canvas;
pub mod font;
pub mod raster;
pub mod theme;
pub mod widgets;

pub use canvas::{Align, Canvas};
