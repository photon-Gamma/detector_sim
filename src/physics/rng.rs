//! 依存なしの小さな乱数生成器 (xorshift64)。
//! 同じ種なら毎回同じ動画になる。

/// 小さい種を 64 ビット全体に広げる (splitmix64)
pub fn mix_seed(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// 種 0 だと xorshift は 0 のまま止まるので、最下位ビットを立てて避ける
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    /// 種を splitmix64 でかき混ぜてから使う版。
    /// `new` は小さい種 (1, 2, …) だと最初の数回の値が 0 に近く偏るので、新しいコードはこちらを使う。
    /// (`new` はフォトダイオードの動画の結果を変えないために残している)
    pub fn seeded(seed: u64) -> Self {
        Rng::new(mix_seed(seed))
    }

    /// [0, 1) の一様乱数
    pub fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// [lo, hi) の一様乱数
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }

    /// 確率 p で true
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_in_unit_interval_even_with_zero_seed() {
        let mut r = Rng::new(0);
        for _ in 0..100_000 {
            let v = r.f();
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn seeded_is_not_biased_for_small_seeds() {
        // 小さい種でも最初の値が偏らない (new だと 0 に近くなる)
        let first: Vec<f32> = (1..200u64).map(|s| Rng::seeded(s).f()).collect();
        let mean = first.iter().sum::<f32>() / first.len() as f32;
        assert!((0.4..0.6).contains(&mean), "mean {mean}");
    }

    #[test]
    fn is_reproducible() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.f().to_bits(), b.f().to_bits());
        }
    }
}
