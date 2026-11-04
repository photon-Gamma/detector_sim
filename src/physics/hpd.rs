//! ハイブリッド光検出器 (HPD) のモデル。
//!
//! 1. 光電面から出た光電子 1 個を高電圧で加速し、真空中を飛ばして APD に打ち込む
//! 2. 打ち込まれた電子はエネルギーを失いながら電子−正孔対を作る (電子打ち込みゲイン G1、平均 200)
//!    → 対の数は平均 200 のポアソン分布とする
//! 3. できた電子 1 個 1 個が APD のなだれ増倍で増える (平均 70、APD の粒子シミュレーションの分布)
//!
//! 合計の増倍率 = Σ (i = 1 … G1) M_i、平均は 200 × 70 ≈ 14000。
//! 2 段の掛け算の過剰雑音係数は F = F1 + (F2 − 1) / 〈G1〉
//! (F1 = 打ち込みの F、F2 = APD の F)。
//!
//! 出典と実機との違い:
//! - 実機の例: 光電面 −8 kV で打ち込みゲイン約 1500、APD 約 80、合計 1.2×10⁵
//!   (浜松ホトニクス「HPD series」TPMH1361E03, 2022)
//! - 電子−正孔対 1 組あたり約 3.6 eV、表面の不感層でエネルギーを失う:
//!   M. Suyama ほか, IEEE Trans. Nucl. Sci. 51, 1056 (2004)
//! - 実際の対の数のゆらぎはポアソンより小さい (Si のファノ因子 0.128, 185 K:
//!   Nucl. Instrum. Methods A 901 (2018), OSTI 1456908)。ここでは簡単のためポアソンとする
//! - 打ち込み 200・APD 70 は指定された説明用の値 (実機より低い)

use super::device::{DeviceSpec, APD70_ALPHA_E};
use super::pmt::poisson;
use super::rng::Rng;
use super::stats::{gain_stats, precompute_gains_for, GainStats};
use std::sync::OnceLock;

/// 電子打ち込みゲインの平均 (設計値)
pub const EB_GAIN: f64 = 200.0;
/// 2 段目の APD の平均増倍率 (設計値。シミュレーションの平均は統計誤差で少しずれる)
pub const APD_DESIGN_GAIN: f64 = 70.0;
/// APD の増倍率の分布として使う、粒子シミュレーションの回数
pub const APD_POOL_SIZE: usize = 100_000;
const APD_POOL_SEED: u64 = 0x4D_0A70;

/// 2 段目の APD (〈M〉≈ 70) の増倍率の標本。重いので 1 回だけ計算して使い回す。
pub fn apd70_pool() -> &'static [u32] {
    static POOL: OnceLock<Vec<u32>> = OnceLock::new();
    POOL.get_or_init(|| {
        eprintln!("  (HPD 用に APD の増倍を {APD_POOL_SIZE} 回計算しています…)");
        precompute_gains_for(&DeviceSpec::apd_with_alpha(APD70_ALPHA_E), APD_POOL_SIZE, APD_POOL_SEED)
    })
}

/// 2 段目の APD の統計 (平均・σ・F)
pub fn apd70_stats() -> Option<GainStats> {
    gain_stats(apd70_pool())
}

/// 打ち込みゲインの過剰雑音係数 (ポアソン分布: F1 = 1 + 1/〈G1〉)
pub fn eb_excess_noise() -> f64 {
    1.0 + 1.0 / EB_GAIN
}

/// 2 段の掛け算の過剰雑音係数 F = F1 + (F2 − 1)/〈G1〉
pub fn cascade_excess_noise(f1: f64, g1: f64, f2: f64) -> f64 {
    if g1 > 0.0 {
        f1 + (f2 - 1.0) / g1
    } else {
        f64::NAN
    }
}

/// HPD の過剰雑音係数の理論値 (2 段目は APD の標本から求めた F を使う)
pub fn excess_noise_theory() -> Option<f64> {
    apd70_stats().map(|s| cascade_excess_noise(eb_excess_noise(), EB_GAIN, s.excess_noise))
}

/// 1 回分の結果
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HpdEvent {
    /// 打ち込みでできた電子の数 (G1)
    pub eb_pairs: u32,
    /// APD で増えた後の合計
    pub total: u64,
}

/// 光電子 1 個ぶんを 1 回計算する。APD の増倍は標本から無作為に選ぶ (独立な電子ごとに 1 つ)
pub fn sample_event(pool: &[u32], rng: &mut Rng) -> HpdEvent {
    let eb_pairs = poisson(rng, EB_GAIN);
    if pool.is_empty() {
        return HpdEvent { eb_pairs, total: 0 };
    }
    let total = (0..eb_pairs)
        .map(|_| {
            let i = ((rng.f() as f64) * pool.len() as f64) as usize;
            u64::from(pool.get(i.min(pool.len() - 1)).copied().unwrap_or(0))
        })
        .sum();
    HpdEvent { eb_pairs, total }
}

/// 合計の増倍率を n 回ぶん求める
pub fn precompute_hpd_gains(n: usize, seed: u64) -> Vec<u32> {
    let pool = apd70_pool();
    let mut rng = Rng::seeded(seed);
    (0..n).map(|_| u32::try_from(sample_event(pool, &mut rng).total).unwrap_or(u32::MAX)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn cascade_formula() {
        // 1 段目にゆらぎがなく (F1 = 1)、2 段目が F2 = 3 で G1 = 2 なら F = 1 + 2/2 = 2
        assert!((cascade_excess_noise(1.0, 2.0, 3.0) - 2.0).abs() < 1e-12);
        assert!(cascade_excess_noise(1.0, 0.0, 3.0).is_nan());
        assert!((eb_excess_noise() - 1.005).abs() < 1e-12);
    }

    /// 合成した分布の平均と F が、理論 (掛け算の平均、F1 + (F2−1)/G1) と合うか
    #[test]
    fn simulated_hpd_matches_cascade_theory() {
        // 重い APD の標本の代わりに、平均 70・F が既知の小さな標本で式を確かめる
        let pool: Vec<u32> = (0..10_000).map(|i| if i % 10 == 0 { 340 } else { 40 }).collect();
        let ps = gain_stats(&pool).unwrap();
        let mut rng = Rng::seeded(5);
        let v: Vec<u32> = (0..40_000).map(|_| sample_event(&pool, &mut rng).total as u32).collect();
        let s = gain_stats(&v).unwrap();
        assert!((s.mean / (EB_GAIN * ps.mean) - 1.0).abs() < 0.01, "mean {}", s.mean);
        let theory = cascade_excess_noise(eb_excess_noise(), EB_GAIN, ps.excess_noise);
        assert!((s.excess_noise - theory).abs() < 0.004, "F {} vs {}", s.excess_noise, theory);
    }
}
