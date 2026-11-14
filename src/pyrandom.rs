//! CPython's `random.Random`, number for number.
//!
//! The OpenSnake server re-checks every speedrun by replaying it in Python, so
//! this game has to make exactly the same random choices from the same seed:
//! the Mersenne Twister (MT19937), Python's way of seeding it from an integer,
//! and the way `randrange`, `choice` and `random` use it.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

#[derive(Clone)]
pub struct PyRandom {
    mt: [u32; N],
    index: usize,
}

impl PyRandom {
    /// `random.Random(seed)` for an integer seed (Python uses its absolute value).
    pub fn new(seed: i64) -> Self {
        let mut n = seed.unsigned_abs();
        let mut key = Vec::new();
        while n > 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        let mut r = PyRandom { mt: [0; N], index: N };
        r.init_by_array(&key);
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32.wrapping_mul(prev ^ (prev >> 30)).wrapping_add(i as u32);
        }
        self.index = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let (mut i, mut j) = (1usize, 0usize);
        let len = key.len();
        for _ in 0..N.max(len) {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= len {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
        }
        self.mt[0] = 0x8000_0000;
    }

    fn generate(&mut self) {
        for kk in 0..N {
            let y = (self.mt[kk] & UPPER_MASK) | (self.mt[(kk + 1) % N] & LOWER_MASK);
            let mut v = self.mt[(kk + M) % N] ^ (y >> 1);
            if y & 1 != 0 {
                v ^= MATRIX_A;
            }
            self.mt[kk] = v;
        }
        self.index = 0;
    }

    /// One 32-bit number (genrand_uint32).
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= N {
            self.generate();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `getrandbits(k)` for k from 1 to 64.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!((1..=64).contains(&k));
        if k <= 32 {
            return (self.next_u32() >> (32 - k)) as u64;
        }
        // Python fills 32-bit words from the least significant end; the last word is cut short.
        let mut out = 0u64;
        let mut bits = k;
        let mut shift = 0;
        while bits > 0 {
            let mut r = self.next_u32();
            if bits < 32 {
                r >>= 32 - bits;
            }
            out |= (r as u64) << shift;
            shift += 32;
            bits = bits.saturating_sub(32);
        }
        out
    }

    /// `_randbelow(n)`: a number from 0 to n - 1, by rejection sampling.
    pub fn randbelow(&mut self, n: u64) -> u64 {
        let k = 64 - n.leading_zeros(); // n.bit_length()
        if k == 0 {
            // (Python asks for 0 bits, which gives 0)
            return 0;
        }
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        r
    }

    /// `randrange(n)`.
    pub fn randrange(&mut self, n: usize) -> usize {
        assert!(n > 0, "empty range for randrange()");
        self.randbelow(n as u64) as usize
    }

    /// `choice(seq)`.
    pub fn choice<'a, T>(&mut self, seq: &'a [T]) -> &'a T {
        assert!(!seq.is_empty(), "cannot choose from an empty sequence");
        &seq[self.randbelow(seq.len() as u64) as usize]
    }

    /// `random()`: a float in [0, 1) with 53 random bits.
    pub fn random(&mut self) -> f64 {
        let a = (self.next_u32() >> 5) as f64;
        let b = (self.next_u32() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }
}
