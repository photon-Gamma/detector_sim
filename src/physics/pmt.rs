//! 光電子増倍管 (PMT) のモデル。
//!
//! - 光電面から出た光電子 1 個が、ダイノードを 1 段ずつ叩いて増える
//! - 1 個の電子がダイノードに当たると、二次電子をポアソン分布 (平均 δ) の個数だけ出す
//! - 段数 n、各段の平均 δ のとき 平均増倍率 = δⁿ
//! - 過剰雑音係数の理論値 F = 1 + 1/δ + 1/δ² + … + 1/δⁿ⁻¹ (ポアソン放出のとき)
//!
//! 出典:
//! - 平均増倍率 μ = δⁿ、δ = A·E^α (α は 0.7〜0.8)、一般的な段数 9〜12:
//!   浜松ホトニクス「Photomultiplier Tubes: Construction and Operating Characteristics」p.7
//! - 多段の雑音 (F の式): W. Shockley, J. R. Pierce, Proc. IRE 26, 321 (1938);
//!   M. C. Teich, K. Matsuo, B. E. A. Saleh, IEEE J. Quantum Electron. 22, 1184 (1986)
//! - 段数 7・δ = 4 は説明用の設計値 (実機の値ではない)
//!
//! [`PmtSim`] は動画用に、電子 1 個 1 個の位置まで追いかける。
//! 統計だけ欲しいときは [`sample_gain`] / [`precompute_pmt_gains`] を使う。

use super::rng::Rng;

/// 増倍の設計値
#[derive(Clone, Copy, Debug)]
pub struct PmtSpec {
    /// ダイノードの段数
    pub stages: usize,
    /// 1 段あたりの平均二次電子数 (設計値のゲイン)
    pub delta: f64,
}

impl PmtSpec {
    /// 7 段、各段のゲイン 4 (平均増倍率 4⁷ = 16384)
    pub const fn standard() -> Self {
        PmtSpec { stages: 7, delta: 4.0 }
    }

    /// 平均増倍率 δⁿ
    pub fn mean_gain(&self) -> f64 {
        self.delta.powi(self.stages as i32)
    }

    /// 過剰雑音係数の理論値 (各段ポアソン放出)
    pub fn excess_noise_theory(&self) -> f64 {
        (0..self.stages).map(|k| self.delta.powi(-(k as i32))).sum()
    }

    /// 理論値 F − 1 のうち、1 段目が占める割合
    pub fn first_stage_share(&self) -> f64 {
        let excess = self.excess_noise_theory() - 1.0;
        if excess > 0.0 {
            (1.0 / self.delta) / excess
        } else {
            0.0
        }
    }
}

/// これより小さい平均は分割して厳密に、これ以上は正規近似で引く
const POISSON_EXACT_LIMIT: f64 = 2000.0;

/// 平均 lambda のポアソン乱数。
/// - λ < 2000: 厳密 (平均 25 以下に分けて Knuth の方法で引き、足し合わせる。
///   独立なポアソン分布の和はポアソン分布なので厳密に正しい)
/// - λ ≥ 2000: 正規近似 (このとき歪度は 1/√λ ≤ 0.02 で、F の計算への影響は無視できる)
pub fn poisson(rng: &mut Rng, lambda: f64) -> u32 {
    if lambda.is_nan() || lambda <= 0.0 || !lambda.is_finite() {
        return 0;
    }
    if lambda < POISSON_EXACT_LIMIT {
        let pieces = (lambda / 25.0).ceil().max(1.0);
        let part = lambda / pieces;
        return (0..pieces as u32).map(|_| poisson_knuth(rng, part)).sum();
    }
    // Box–Muller で正規乱数を作り、平均 λ・分散 λ の正規分布で近似
    let u1 = f64::from(rng.f()).max(1e-12);
    let u2 = f64::from(rng.f());
    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    (lambda + z * lambda.sqrt()).round().max(0.0) as u32
}

/// Knuth の方法 (λ が小さいとき用。λ ≤ 30 程度で使う)
fn poisson_knuth(rng: &mut Rng, lambda: f64) -> u32 {
    let limit = (-lambda).exp();
    let mut k = 0u32;
    let mut p = 1.0f64;
    loop {
        p *= f64::from(rng.f()).max(1e-12);
        if p <= limit || k >= 1000 {
            return k;
        }
        k += 1;
    }
}

/// 光電子 1 個から始めた増倍率を 1 回ぶん求める
pub fn sample_gain(spec: &PmtSpec, rng: &mut Rng) -> u32 {
    let mut n: u32 = 1;
    for _ in 0..spec.stages {
        // 電子 1 個ずつポアソン分布で二次電子を出す (独立なポアソン分布の和もポアソン分布)
        n = poisson(rng, spec.delta * f64::from(n));
        if n == 0 {
            break;
        }
    }
    n
}

/// 増倍率を n 回ぶん求める (ヒストグラムの事前計算)
pub fn precompute_pmt_gains(spec: &PmtSpec, n: usize, seed: u64) -> Vec<u32> {
    let mut rng = Rng::seeded(seed);
    (0..n).map(|_| sample_gain(spec, &mut rng)).collect()
}

// ------------------------------------------------------------ 動画用の粒子シミュレーション

/// ノード番号: 0 = 光電面, 1..=stages = ダイノード, stages+1 = アノード
pub type Node = usize;

/// ノード間を飛んでいる電子
#[derive(Clone, Copy, Debug)]
pub struct Electron {
    /// 出発したノード (到着先は from + 1)
    pub from: Node,
    /// 進み具合 0..1 (負の間は出発待ち)
    pub t: f32,
    /// ビームの中での横位置 −1..1 (広がりの表示用)
    pub lane: f32,
}

/// 動画用の PMT シミュレーション。光電子を1個入れると、段ごとに増える様子を追える。
pub struct PmtSim {
    spec: PmtSpec,
    rng: Rng,
    electrons: Vec<Electron>,
    /// ノードごとの「到着した電子の累計」
    arrivals: Vec<u64>,
    /// 直前のステップで各ノードに到着した数 (光らせる表示用)
    hits: Vec<u32>,
    /// アノードで集めた電子数 (1 回の増倍が終わるたびに確定)
    finished: Vec<u64>,
    in_progress: bool,
}

/// ノード間を飛ぶのにかかるフレーム数 (速さ 1 のとき)
pub const LEG_FRAMES: f32 = 36.0;
/// 飛行中の電子数の上限 (メモリ保護。標準の 4096 倍には十分)
const MAX_ELECTRONS: usize = 200_000;

impl PmtSim {
    pub fn new(spec: PmtSpec, seed: u64) -> Self {
        let nodes = spec.stages + 2;
        PmtSim {
            spec,
            rng: Rng::seeded(seed),
            electrons: Vec::new(),
            arrivals: vec![0; nodes],
            hits: vec![0; nodes],
            finished: Vec::new(),
            in_progress: false,
        }
    }

    pub fn spec(&self) -> &PmtSpec {
        &self.spec
    }
    pub fn electrons(&self) -> &[Electron] {
        &self.electrons
    }
    /// ノードごとの到着数 (今の 1 回分)
    pub fn arrivals(&self) -> &[u64] {
        &self.arrivals
    }
    pub fn hits(&self) -> &[u32] {
        &self.hits
    }
    /// これまでに終わった各回の増倍率
    pub fn finished(&self) -> &[u64] {
        &self.finished
    }
    pub fn busy(&self) -> bool {
        self.in_progress
    }
    pub fn anode(&self) -> Node {
        self.spec.stages + 1
    }

    /// 光電面から光電子を 1 個出す (前の回の途中なら何もしない)
    pub fn emit_photoelectron(&mut self) {
        if self.in_progress {
            return;
        }
        self.arrivals.iter_mut().for_each(|a| *a = 0);
        self.arrivals[0] = 1;
        self.electrons.push(Electron { from: 0, t: 0.0, lane: 0.0 });
        self.in_progress = true;
    }

    /// dt フレーム進める (dt < 1 でスロー)
    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 8.0) } else { 0.0 };
        self.hits.iter_mut().for_each(|h| *h = 0);
        let anode = self.anode();
        let mut arrived: Vec<Node> = Vec::new();
        self.electrons.retain_mut(|e| {
            e.t += dt / LEG_FRAMES;
            if e.t >= 1.0 {
                arrived.push(e.from + 1);
                false
            } else {
                true
            }
        });
        for node in arrived {
            if let Some(a) = self.arrivals.get_mut(node) {
                *a += 1;
            }
            if let Some(h) = self.hits.get_mut(node) {
                *h = h.saturating_add(1);
            }
            if node < anode {
                // ダイノードに当たった: 二次電子を放出
                let k = poisson(&mut self.rng, self.spec.delta);
                for _ in 0..k {
                    if self.electrons.len() >= MAX_ELECTRONS {
                        break;
                    }
                    let lane = self.rng.range(-1.0, 1.0);
                    let wait = -self.rng.f() * 0.12; // 少しずつずらして出発
                    self.electrons.push(Electron { from: node, t: wait, lane });
                }
            }
        }
        if self.in_progress && self.electrons.is_empty() {
            self.in_progress = false;
            self.finished.push(self.arrivals.get(anode).copied().unwrap_or(0));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::physics::stats::gain_stats;

    #[test]
    fn theory_values_for_standard_pmt() {
        let s = PmtSpec::standard();
        assert_eq!(s.stages, 7);
        assert_eq!(s.mean_gain(), 16384.0);
        assert!((s.excess_noise_theory() - 1.333).abs() < 1e-3);
        assert!((s.first_stage_share() - 0.75).abs() < 0.01);
    }

    #[test]
    fn poisson_has_right_mean_and_variance() {
        let mut r = Rng::seeded(9);
        for lambda in [0.5, 4.0, 100.0, 200.0, 1500.0, 5000.0] {
            let v: Vec<f64> = (0..20_000).map(|_| f64::from(poisson(&mut r, lambda))).collect();
            let m = v.iter().sum::<f64>() / v.len() as f64;
            let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
            assert!((m - lambda).abs() < 0.05 * lambda + 0.05, "λ={lambda} mean={m}");
            assert!((var - lambda).abs() < 0.1 * lambda + 0.05, "λ={lambda} var={var}");
        }
        assert_eq!(poisson(&mut r, f64::NAN), 0);
        assert_eq!(poisson(&mut r, -1.0), 0);
    }

    #[test]
    fn sampled_gain_matches_theory() {
        let spec = PmtSpec::standard();
        let v = precompute_pmt_gains(&spec, 20_000, 3);
        let s = gain_stats(&v).unwrap();
        assert!((s.mean / spec.mean_gain() - 1.0).abs() < 0.03, "mean {}", s.mean);
        assert!((s.excess_noise - spec.excess_noise_theory()).abs() < 0.05, "F {}", s.excess_noise);
    }

    #[test]
    fn particle_sim_counts_match_cascade() {
        let mut p = PmtSim::new(PmtSpec::standard(), 11);
        p.emit_photoelectron();
        for _ in 0..10_000 {
            if !p.busy() {
                break;
            }
            p.step(1.0);
        }
        assert!(!p.busy());
        assert_eq!(p.finished().len(), 1);
        assert_eq!(p.finished()[0], p.arrivals()[p.anode()]);
        assert!(p.electrons().is_empty());
    }
}
