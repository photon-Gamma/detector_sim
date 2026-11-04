//! 光子ごとの利得 M の統計。

use super::device::DeviceSpec;
use super::sim::{Sim, STATS_CARRIER_LIMIT};

#[derive(Clone, Copy, Debug)]
pub struct GainStats {
    pub n: usize,
    /// 平均 〈M〉
    pub mean: f64,
    /// 標準偏差 σ (不偏)
    pub sd: f64,
    /// 過剰雑音係数 F = 〈M²〉/〈M〉²
    pub excess_noise: f64,
    pub median: u32,
    pub max: u32,
}

/// 2個以上のデータがあり、平均が正のときだけ統計を返す
pub fn gain_stats(v: &[u32]) -> Option<GainStats> {
    if v.len() < 2 {
        return None;
    }
    let n = v.len() as f64;
    let mean = v.iter().map(|&x| f64::from(x)).sum::<f64>() / n;
    if mean <= 0.0 {
        return None;
    }
    let m2 = v.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>() / n;
    let var = v.iter().map(|&x| (f64::from(x) - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let mut sorted = v.to_vec();
    sorted.sort_unstable();
    Some(GainStats {
        n: v.len(),
        mean,
        sd: var.sqrt(),
        excess_noise: m2 / (mean * mean),
        median: sorted.get(sorted.len() / 2).copied().unwrap_or(0),
        max: sorted.last().copied().unwrap_or(0),
    })
}

/// 実数データのばらつきの要約 (標準偏差・歪度)
#[derive(Clone, Copy, Debug)]
pub struct Describe {
    /// 標準偏差 (不偏)
    pub sd: f64,
    /// 歪度 (0 なら左右対称。正規分布は 0)
    pub skew: f64,
}

pub fn describe(v: &[f64]) -> Option<Describe> {
    if v.len() < 3 || v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let m2 = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let m3 = v.iter().map(|x| (x - mean).powi(3)).sum::<f64>() / n;
    let skew = if m2 > 0.0 { m3 / m2.powf(1.5) } else { 0.0 };
    Some(Describe { sd: (m2 * n / (n - 1.0)).sqrt(), skew })
}

/// pool を n 個ずつの重ならない組に分け、各組の平均を最大 batches 個返す。
/// (足りなければ作れる分だけ)
pub fn batch_means(pool: &[u32], n: usize, batches: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    pool.chunks_exact(n)
        .take(batches)
        .map(|c| c.iter().map(|&m| f64::from(m)).sum::<f64>() / n as f64)
        .collect()
}

/// 1回の試行で何ステップまで待つか (これを超える試行は捨てる)
const MAX_STEPS_PER_TRIAL: usize = 50_000;

/// 1光子ずつ APD に入れ、光子ごとの利得 M を n 回分集める (ヒストグラムの事前計算)
pub fn precompute_gains(n: usize, seed: u64) -> Vec<u32> {
    precompute_gains_for(&DeviceSpec::apd(), n, seed)
}

/// precompute_gains の APD の設定を指定できる版
pub fn precompute_gains_for(spec: &DeviceSpec, n: usize, seed: u64) -> Vec<u32> {
    let mut out = Vec::with_capacity(n);
    let mut trial = 0u64;
    // 何かの理由で試行が終わらなくても、試行回数の上限で必ず止まる
    while out.len() < n && trial < (n as u64).saturating_mul(2) + 10 {
        trial += 1;
        let mut s = Sim::new(spec.clone(), (seed ^ trial).wrapping_mul(0x9E37_79B9_7F4A_7C15)).with_carrier_limit(STATS_CARRIER_LIMIT);
        s.emit(0.5, None);
        for _ in 0..MAX_STEPS_PER_TRIAL {
            if let Some(&m) = s.gains().first() {
                out.push(m);
                break;
            }
            s.step(1.0);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stats_of_constant_gain() {
        let s = gain_stats(&[1, 1, 1, 1]).unwrap();
        assert_eq!(s.mean, 1.0);
        assert_eq!(s.sd, 0.0);
        assert_eq!(s.excess_noise, 1.0);
    }

    #[test]
    fn stats_need_enough_data() {
        assert!(gain_stats(&[]).is_none());
        assert!(gain_stats(&[5]).is_none());
        assert!(gain_stats(&[0, 0]).is_none());
    }

    #[test]
    fn excess_noise_is_at_least_one() {
        // F = 1 + σ²/〈M〉² (母分散) なので常に 1 以上
        let s = gain_stats(&[1, 4, 9, 30, 2]).unwrap();
        assert!(s.excess_noise >= 1.0);
    }

    #[test]
    fn batch_means_are_non_overlapping() {
        let pool: Vec<u32> = (1..=10).collect();
        assert_eq!(batch_means(&pool, 5, 10), vec![3.0, 8.0]);
        assert_eq!(batch_means(&pool, 3, 2), vec![2.0, 5.0]);
        assert!(batch_means(&pool, 0, 5).is_empty());
        assert!(batch_means(&pool, 11, 5).is_empty());
    }

    #[test]
    fn describe_symmetric_data() {
        let d = describe(&[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();
        assert!((d.sd - 2.5f64.sqrt()).abs() < 1e-12);
        assert!(d.skew.abs() < 1e-12);
        assert!(describe(&[1.0, f64::NAN, 2.0]).is_none());
    }

    #[test]
    fn apd70_setting_gives_mean_gain_near_70() {
        use crate::physics::device::APD70_ALPHA_E;
        let v = precompute_gains_for(&DeviceSpec::apd_with_alpha(APD70_ALPHA_E), 5000, 77);
        let m = gain_stats(&v).unwrap().mean;
        assert!((60.0..80.0).contains(&m), "mean {m}");
    }

    #[test]
    fn precompute_returns_requested_count() {
        let v = precompute_gains(200, 1);
        assert_eq!(v.len(), 200);
        assert!(v.iter().all(|&m| m >= 1));
    }
}
