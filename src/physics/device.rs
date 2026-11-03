//! 検出器の構造と物理パラメータ。
//!
//! 深さ d は 0 (表面・p側) 〜 1 (裏面・n側) に規格化してある。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    SiPin,
    Apd,
}

/// 層の種類 (名前や色は描画側で決める)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    PPlus,
    Intrinsic,
    Pi,
    PMult,
    NPlus,
}

#[derive(Clone, Copy, Debug)]
pub struct Layer {
    pub d0: f32,
    pub d1: f32,
    pub kind: LayerKind,
}

#[derive(Clone, Debug)]
pub struct DeviceSpec {
    pub kind: DeviceKind,
    pub layers: Vec<Layer>,
    /// 光が吸収される範囲
    pub absorb: (f32, f32),
    /// 衝突電離が起きる範囲 (APD のみ)
    pub mult: Option<(f32, f32)>,
    /// 正孔が集められる深さ (これより浅くなったら p側電極へ)
    pub p_edge: f32,
    /// 電子が集められる深さ (これより深くなったら n側電極へ)
    pub n_edge: f32,
    /// 電子の衝突電離率 (1フレームあたりの確率)
    pub alpha_e: f32,
    /// 正孔の衝突電離率
    pub alpha_h: f32,
}

/// 電子と正孔の電離率の比 (1 フレームあたりの値で alpha_e / alpha_h)
pub const APD_ALPHA_RATIO: f32 = 30.0;

/// 平均利得が約 70 になる電子の電離率 (PMT との比較用。tests で確認)
pub const APD70_ALPHA_E: f32 = 0.1388;

impl DeviceSpec {
    /// Si PIN フォトダイオード: p⁺ / i / n⁺
    pub fn si_pin() -> Self {
        DeviceSpec {
            kind: DeviceKind::SiPin,
            layers: vec![
                Layer { d0: 0.0, d1: 0.08, kind: LayerKind::PPlus },
                Layer { d0: 0.08, d1: 0.88, kind: LayerKind::Intrinsic },
                Layer { d0: 0.88, d1: 1.0, kind: LayerKind::NPlus },
            ],
            absorb: (0.08, 0.88),
            mult: None,
            p_edge: 0.08,
            n_edge: 0.88,
            alpha_e: 0.0,
            alpha_h: 0.0,
        }
    }

    /// リーチスルー型 APD: p⁺ / π (吸収) / p (増倍) / n⁺ (平均利得 ≈ 22.5)
    pub fn apd() -> Self {
        Self::apd_with_alpha(0.12)
    }

    /// 電子の電離率 alpha_e を変えた APD (逆バイアスを変えたことに相当)。
    /// 正孔の電離率は alpha_e / 30 (電離率の比を一定に保つ)。
    pub fn apd_with_alpha(alpha_e: f32) -> Self {
        let alpha_e = if alpha_e.is_finite() { alpha_e.clamp(0.0, 1.0) } else { 0.0 };
        DeviceSpec {
            kind: DeviceKind::Apd,
            layers: vec![
                Layer { d0: 0.0, d1: 0.06, kind: LayerKind::PPlus },
                Layer { d0: 0.06, d1: 0.58, kind: LayerKind::Pi },
                Layer { d0: 0.58, d1: 0.86, kind: LayerKind::PMult },
                Layer { d0: 0.86, d1: 1.0, kind: LayerKind::NPlus },
            ],
            absorb: (0.06, 0.58),
            mult: Some((0.60, 0.86)),
            p_edge: 0.06,
            n_edge: 0.86,
            alpha_e,
            alpha_h: alpha_e / APD_ALPHA_RATIO,
        }
    }

    /// 電界強度 (表示用に 0..1 に規格化)
    pub fn field(&self, d: f32) -> f32 {
        match self.kind {
            DeviceKind::SiPin => {
                if !(0.06..=0.90).contains(&d) {
                    0.0
                } else if d < 0.08 {
                    (d - 0.06) / 0.02 * 0.35
                } else if d > 0.88 {
                    (0.90 - d) / 0.02 * 0.35
                } else {
                    0.35
                }
            }
            DeviceKind::Apd => {
                if !(0.04..=0.88).contains(&d) {
                    0.0
                } else if d < 0.06 {
                    (d - 0.04) / 0.02 * 0.15
                } else if d < 0.58 {
                    0.15
                } else if d < 0.86 {
                    0.15 + (d - 0.58) / 0.28 * 0.85
                } else {
                    (0.88 - d) / 0.02
                }
            }
        }
    }

    pub fn in_mult(&self, d: f32) -> bool {
        matches!(self.mult, Some((m0, m1)) if d > m0 && d < m1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_cover_device_without_gaps() {
        for spec in [DeviceSpec::si_pin(), DeviceSpec::apd()] {
            assert_eq!(spec.layers.first().map(|l| l.d0), Some(0.0));
            assert_eq!(spec.layers.last().map(|l| l.d1), Some(1.0));
            for w in spec.layers.windows(2) {
                assert_eq!(w[0].d1, w[1].d0);
            }
            assert!(spec.p_edge < spec.n_edge);
        }
    }

    #[test]
    fn field_is_bounded() {
        for spec in [DeviceSpec::si_pin(), DeviceSpec::apd()] {
            for k in -10..=110 {
                let e = spec.field(k as f32 / 100.0);
                assert!((0.0..=1.0).contains(&e), "E={e}");
            }
        }
    }
}
