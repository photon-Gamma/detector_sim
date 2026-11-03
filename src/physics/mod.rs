//! 物理モデル (描画とは独立)。
//!
//! - `device`: 検出器の層構造・電界・パラメータ
//! - `sim`:    光子・電子・正孔の粒子シミュレーション
//! - `stats`:  光子ごとの利得 M の統計
//! - `pmt`:    光電子増倍管 (PMT) のダイノード増倍
//! - `hpd`:    ハイブリッド光検出器 (HPD): 電子打ち込み + APD の 2 段増倍
//! - `rng`:    再現性のある乱数

pub mod device;
pub mod hpd;
pub mod pmt;
pub mod rng;
pub mod sim;
pub mod stats;

pub use device::DeviceSpec;
pub use rng::{mix_seed, Rng};
pub use sim::Sim;
