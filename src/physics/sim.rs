//! 光子・電子・正孔の粒子シミュレーション (教育用の簡略モデル)。
//!
//! - 光子は吸収層で指数分布の深さで吸収され、電子−正孔対を1組生成
//! - 逆バイアス電界で 電子→n側, 正孔→p側 に一定速度でドリフト
//! - 増倍領域では 1フレームあたり確率 α·dt で衝突電離 → 新たな電子−正孔対
//! - 外部電流は Ramo の定理 i = Σ q·v / W (1対の電荷を時間積分すると q)
//! - キャリアに「どの光子由来か」を持たせ、光子ごとの利得 M を数える

use super::device::DeviceSpec;
use super::rng::Rng;
use std::collections::{HashMap, VecDeque};

/// 電子のドリフト速度 (深さ/フレーム)
pub const VE: f32 = 0.012;
/// 正孔のドリフト速度
pub const VH: f32 = 0.007;
/// 光子の表示上の速度
pub const VP: f32 = 0.035;
/// 光子の出発位置 (デバイスの上の空中)
pub const PHOTON_START: f32 = -0.22;
/// 電流履歴の長さ (グラフの横幅ぶん)
pub const HISTORY_LEN: usize = 240;
/// キャリア数の上限 (動画用)。超えたら新しい衝突電離を止める (メモリと描画時間の保護)
pub const MAX_CARRIERS: usize = 3000;
/// 統計計算用の上限。描画しないので大きくして、なだれを途中で止めないようにする
pub const STATS_CARRIER_LIMIT: usize = 200_000;
/// 1ステップの時間刻みの上限
const MAX_DT: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub struct Carrier {
    pub x: f32,
    pub d: f32,
    pub elec: bool,
    /// 生まれた元の光子の ID
    pub pid: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Photon {
    pub x: f32,
    pub d: f32,
    pub target: f32,
    pub pid: u32,
}

/// 吸収・衝突電離の瞬間の光る演出
#[derive(Clone, Copy, Debug)]
pub struct Flash {
    pub x: f32,
    pub d: f32,
    pub t: f32,
    pub ion: bool,
}
pub const FLASH_LIFE: f32 = 18.0;

/// 累計カウンタ
#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub absorbed: u32,
    pub collected: u32,
    pub ionizations: u32,
}

/// 光子ごとの途中経過
#[derive(Clone, Copy, Debug)]
struct Track {
    in_flight: u32,
    electrons: u32,
}

pub struct Sim {
    spec: DeviceSpec,
    rng: Rng,
    carriers: Vec<Carrier>,
    photons: Vec<Photon>,
    flashes: Vec<Flash>,
    counts: Counts,
    history: VecDeque<f32>,
    next_pid: u32,
    tracks: HashMap<u32, Track>,
    gains: Vec<u32>,
    /// gains と同じ順で、その光子の ID
    gain_pids: Vec<u32>,
    /// reset_counts した時点の光子 ID (これより後に入った光子だけを数えるため)
    pid_mark: u32,
    carrier_limit: usize,
}

impl Sim {
    pub fn new(spec: DeviceSpec, seed: u64) -> Self {
        Sim {
            spec,
            rng: Rng::new(seed),
            carriers: Vec::new(),
            photons: Vec::new(),
            flashes: Vec::new(),
            counts: Counts::default(),
            history: VecDeque::from(vec![0.0; HISTORY_LEN]),
            next_pid: 0,
            tracks: HashMap::new(),
            gains: Vec::new(),
            gain_pids: Vec::new(),
            pid_mark: 0,
            carrier_limit: MAX_CARRIERS,
        }
    }

    /// キャリア数の上限を変える (統計計算用)
    pub fn with_carrier_limit(mut self, limit: usize) -> Self {
        self.carrier_limit = limit.clamp(2, STATS_CARRIER_LIMIT);
        self
    }

    // ------------------------------------------------ 読み出し (描画・統計用)
    pub fn spec(&self) -> &DeviceSpec {
        &self.spec
    }
    pub fn carriers(&self) -> &[Carrier] {
        &self.carriers
    }
    pub fn photons(&self) -> &[Photon] {
        &self.photons
    }
    pub fn flashes(&self) -> &[Flash] {
        &self.flashes
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    /// 外部電流の履歴 (古い順)
    pub fn history(&self) -> &VecDeque<f32> {
        &self.history
    }
    /// 増倍が終わった光子ごとの利得 M (確定順)
    pub fn gains(&self) -> &[u32] {
        &self.gains
    }
    /// 飛行中または増倍中の光子の数
    pub fn photons_in_progress(&self) -> usize {
        self.photons.len() + self.tracks.len()
    }

    // ------------------------------------------------ 操作
    pub fn reset_counts(&mut self) {
        self.counts = Counts::default();
        self.pid_mark = self.next_pid;
    }

    /// reset_counts の後に入射し、M が確定した光子の利得
    /// (reset 前の光子が後から確定しても混ぜない: 表示の「吸収した光子」と数をそろえるため)
    pub fn recent_gains(&self) -> Vec<u32> {
        self.gains.iter().zip(&self.gain_pids).filter(|(_, &p)| p > self.pid_mark).map(|(&g, _)| g).collect()
    }

    /// 光子を1個入射させる。target は吸収される深さ (None なら指数分布で決める)
    pub fn emit(&mut self, x: f32, target: Option<f32>) {
        let (a0, a1) = self.spec.absorb;
        let target = match target {
            Some(t) if t.is_finite() => t.clamp(a0, a1),
            _ => self.sample_depth(),
        };
        self.next_pid = self.next_pid.wrapping_add(1);
        self.photons.push(Photon { x: x.clamp(0.0, 1.0), d: PHOTON_START, target, pid: self.next_pid });
    }

    /// 横位置もランダムに光子を入射
    pub fn emit_random(&mut self) {
        let x = 0.18 + self.rng.f() * 0.64;
        self.emit(x, None);
    }

    /// 吸収深さ: 吸収層の上面から指数分布 (層を抜けたら引き直す)
    fn sample_depth(&mut self) -> f32 {
        let (a0, a1) = self.spec.absorb;
        for _ in 0..64 {
            let u = self.rng.f().max(1e-4);
            let d = a0 + 0.01 - u.ln() * 0.30;
            if d < a1 - 0.02 {
                return d;
            }
        }
        (a0 + a1) / 2.0 // 万一引き直しが続いたら中央で吸収
    }

    /// 時間を dt フレームぶん進める (dt<1 でスロー再生)
    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, MAX_DT) } else { 0.0 };
        self.move_photons(dt);
        self.drift_and_ionize(dt);
        let current = self.current();
        self.collect();
        for f in self.flashes.iter_mut() {
            f.t += dt;
        }
        self.flashes.retain(|f| f.t < FLASH_LIFE);
        self.history.pop_front();
        self.history.push_back(current);
    }

    fn move_photons(&mut self, dt: f32) {
        let mut absorbed = Vec::new();
        self.photons.retain_mut(|p| {
            p.d += VP * dt;
            if p.d >= p.target {
                absorbed.push(*p);
                false
            } else {
                true
            }
        });
        for p in absorbed {
            self.counts.absorbed = self.counts.absorbed.saturating_add(1);
            self.flashes.push(Flash { x: p.x, d: p.target, t: 0.0, ion: false });
            self.carriers.push(Carrier { x: p.x - 0.008, d: p.target, elec: true, pid: p.pid });
            self.carriers.push(Carrier { x: p.x + 0.008, d: p.target, elec: false, pid: p.pid });
            self.tracks.insert(p.pid, Track { in_flight: 2, electrons: 0 });
        }
    }

    fn drift_and_ionize(&mut self, dt: f32) {
        let mut born = Vec::new();
        let room = self.carriers.len() < self.carrier_limit;
        for c in self.carriers.iter_mut() {
            if c.elec {
                c.d += VE * dt;
            } else {
                c.d -= VH * dt;
            }
            if room && self.spec.in_mult(c.d) {
                let alpha = if c.elec { self.spec.alpha_e } else { self.spec.alpha_h };
                if self.rng.f() < alpha * dt {
                    // 新しい対は少し横にずらして、なだれが枝分かれして見えるようにする
                    let j = 0.02 + self.rng.f() * 0.035;
                    let s = if self.rng.f() < 0.5 { -1.0 } else { 1.0 };
                    born.push((c.x + s * j, c.d, c.pid));
                    c.x = (c.x - s * j * 0.5).clamp(0.03, 0.97);
                }
            }
        }
        for (x, d, pid) in born {
            let x = x.clamp(0.03, 0.97);
            self.counts.ionizations = self.counts.ionizations.saturating_add(1);
            self.flashes.push(Flash { x, d, t: 0.0, ion: true });
            self.carriers.push(Carrier { x, d, elec: true, pid });
            self.carriers.push(Carrier { x, d, elec: false, pid });
            if let Some(t) = self.tracks.get_mut(&pid) {
                t.in_flight = t.in_flight.saturating_add(2);
            }
        }
    }

    /// Ramo の定理による外部電流 (実時間の速度で計算するのでスローでも値は同じ)
    fn current(&self) -> f32 {
        let w = (self.spec.n_edge - self.spec.p_edge).max(1e-3);
        self.carriers.iter().map(|c| if c.elec { VE } else { VH }).sum::<f32>() / w
    }

    /// 電極に着いたキャリアを取り除き、光子ごとの利得を確定させる
    fn collect(&mut self) {
        let (pe, ne) = (self.spec.p_edge, self.spec.n_edge);
        let tracks = &mut self.tracks;
        let gains = &mut self.gains;
        let gain_pids = &mut self.gain_pids;
        let counts = &mut self.counts;
        self.carriers.retain(|c| {
            let e_out = c.elec && c.d >= ne;
            let h_out = !c.elec && c.d <= pe;
            if !(e_out || h_out) {
                return true;
            }
            if e_out {
                counts.collected = counts.collected.saturating_add(1);
            }
            if let Some(t) = tracks.get_mut(&c.pid) {
                t.in_flight = t.in_flight.saturating_sub(1);
                if e_out {
                    t.electrons = t.electrons.saturating_add(1);
                }
                // その光子由来のキャリアが全部抜けたら利得 M が確定
                if t.in_flight == 0 {
                    gains.push(t.electrons);
                    gain_pids.push(c.pid);
                    tracks.remove(&c.pid);
                }
            }
            false
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn run_until_idle(s: &mut Sim) {
        for _ in 0..100_000 {
            if s.photons_in_progress() == 0 {
                return;
            }
            s.step(1.0);
        }
        unreachable!("シミュレーションが終わらない");
    }

    #[test]
    fn si_pin_gain_is_exactly_one() {
        let mut s = Sim::new(DeviceSpec::si_pin(), 7);
        for k in 0..50 {
            s.emit(0.2 + k as f32 * 0.01, None);
        }
        run_until_idle(&mut s);
        assert_eq!(s.gains().len(), 50);
        assert!(s.gains().iter().all(|&m| m == 1));
        assert!(s.carriers().is_empty());
    }

    #[test]
    fn apd_charge_is_conserved() {
        // 衝突電離1回で電子が1個増えるので、集めた電子 = 光子数 + 電離回数
        let mut s = Sim::new(DeviceSpec::apd(), 99);
        for _ in 0..30 {
            s.emit_random();
        }
        run_until_idle(&mut s);
        let c = s.counts();
        assert_eq!(c.collected, c.absorbed + c.ionizations);
        assert_eq!(s.gains().iter().sum::<u32>(), c.collected);
    }

    #[test]
    fn recent_gains_skip_photons_from_before_reset() {
        let mut s = Sim::new(DeviceSpec::si_pin(), 21);
        s.emit(0.5, Some(0.5));
        for _ in 0..20 {
            s.step(1.0);
        }
        s.reset_counts(); // 1 個目はまだ走っている
        s.emit(0.5, Some(0.5));
        run_until_idle(&mut s);
        assert_eq!(s.gains().len(), 2);
        assert_eq!(s.recent_gains(), vec![1]);
    }

    #[test]
    fn bad_inputs_do_not_break_the_simulation() {
        let mut s = Sim::new(DeviceSpec::apd(), 3);
        s.emit(f32::NAN, Some(f32::INFINITY));
        s.emit(-5.0, Some(99.0));
        for dt in [f32::NAN, -1.0, 1e9, 1.0] {
            s.step(dt);
        }
        run_until_idle(&mut s);
        assert_eq!(s.gains().len(), 2);
    }

    #[test]
    fn carrier_count_is_capped() {
        let mut spec = DeviceSpec::apd();
        spec.alpha_e = 1.0;
        spec.alpha_h = 1.0; // 暴走するなだれ
        let mut s = Sim::new(spec, 5);
        s.emit(0.5, Some(0.5));
        for _ in 0..2000 {
            s.step(1.0);
            assert!(s.carriers().len() <= MAX_CARRIERS * 3);
        }
    }
}
