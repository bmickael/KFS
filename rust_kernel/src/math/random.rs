use super::convert::Convert;

use super::MathError;
use super::MathResult;

mod rdrand;
use rdrand::rdrand;

mod lfsr16;
use lfsr16::{lfsr16_get_seed, lfsr16_srand_init, GetPseudoNumber};

use bit_field::BitField;

/// Has provide two methods
/// rand is totally undetermined and use RDRAND cpu feature (ivybridge +)
/// srand is seeded based random and use a seed algorythm
pub trait Random<T> {
    fn rand(self) -> T;
    fn srand(self) -> T;
}

/// Enumeration of all randomize methods
pub enum Methods {
    Rdrand,
    Lfsr16,
}

/// internal trait, Randup (not roundup) is a common family name in US
pub trait Rand<T> {
    fn randup(self, method: Methods) -> T;
}

/// For now, lfsr16 is the only one method for srand, implentation may be extended in future
pub fn srand_init(seed: u16) -> MathResult<()> {
    lfsr16_srand_init(seed)
}

/// Main trait inherance
impl<T: Rand<T>> Random<T> for T {
    fn rand(self) -> T {
        T::randup(self, Methods::Rdrand)
    }
    fn srand(self) -> T {
        match lfsr16_get_seed() {
            Ok(_) => T::randup(self, Methods::Lfsr16),
            Err(e) => panic!("{} {:?}", function!(), e),
        }
    }
}

trait Generate<T> {
    /// get a random number with the right method
    fn generate(method: Methods) -> T;
}

impl Generate<u32> for u32 {
    fn generate(method: Methods) -> u32 {
        match method {
            Methods::Rdrand => rdrand(),
            Methods::Lfsr16 => u32::get_pseudo_number(),
        }
    }
}

impl Generate<i32> for i32 {
    fn generate(method: Methods) -> i32 {
        match method {
            Methods::Rdrand => rdrand(),
            Methods::Lfsr16 => i32::get_pseudo_number(),
        }
    }
}

/// f64 rand: -self..+self as f64
impl Rand<f64> for f64 {
    /// [core::i32::MIN..core::i32::MAX] € Z -> [+1..~-1] € D -> [+self..-self] € D
    fn randup(self, method: Methods) -> f64 {
        let t: i32 = i32::generate(method);
        t as f64 / core::i32::MIN as f64 * self as f64
    }
}

/// f32 rand: -self..+self as f32
impl Rand<f32> for f32 {
    /// [core::i32::MIN..core::i32::MAX] € Z -> [+1..~-1] € D -> [+self..-self] € D
    fn randup(self, method: Methods) -> f32 {
        let t: i32 = i32::generate(method);
        t as f32 / core::i32::MIN as f32 * self as f32
    }
}

/// i32 rand: -self..+self as i32
impl Rand<i32> for i32 {
    /// [core::i32::MIN..core::i32::MAX] € Z -> [+1..~-1] € D -> [+self..-self] € D -> [+self..-self] € Z
    fn randup(self, method: Methods) -> i32 {
        let t: i32 = i32::generate(method);
        // lack of precision for i32 type with f32, usage of f64 instead
        (t as f64 / core::i32::MIN as f64 * self as f64).round() as i32
    }
}

/// i16 rand: -self..+self as i16
impl Rand<i16> for i16 {
    /// [core::i32::MIN..core::i32::MAX] € Z -> [+1..~-1] € D -> [+self..-self] € D -> [+self..-self] € Z
    fn randup(self, method: Methods) -> i16 {
        let t: i32 = i32::generate(method);
        (t as f32 / core::i32::MIN as f32 * self as f32).round() as i16
    }
}

/// i8 rand: -self..+self as i8
impl Rand<i8> for i8 {
    /// [core::i32::MIN..core::i32::MAX] € Z -> [+1..~-1] € D -> [+self..-self] € D -> [+self..-self] € Z
    fn randup(self, method: Methods) -> i8 {
        let t: i32 = i32::generate(method);
        (t as f32 / core::i32::MIN as f32 * self as f32).round() as i8
    }
}

/// u32 rand: 0..+self as u32
impl Rand<u32> for u32 {
    /// [0..core::u32::MAX] € N -> [0..+1] € D -> [0..+self] € D -> [0..+self] € N
    fn randup(self, method: Methods) -> u32 {
        let t: u32 = u32::generate(method);
        // lack of precision for u32 type with f32, usage of f64 instead
        (t as f64 / core::u32::MAX as f64 * self as f64).round() as u32
    }
}

/// u16 rand: 0..+self as u16
impl Rand<u16> for u16 {
    /// [0..core::u32::MAX] € N -> [0..+1] € D -> [0..+self] € D -> [0..+self] € N
    fn randup(self, method: Methods) -> u16 {
        let t: u32 = u32::generate(method);
        (t as f32 / core::u32::MAX as f32 * self as f32).round() as u16
    }
}

/// u8 rand: 0..+self as u8
impl Rand<u8> for u8 {
    /// [0..core::u32::MAX] € N -> [0..+1] € D -> [0..+self] € D -> [0..+self] € N
    fn randup(self, method: Methods) -> u8 {
        let t: u32 = u32::generate(method);
        (t as f32 / core::u32::MAX as f32 * self as f32).round() as u8
    }
}

/// bool rand: 0..1 as bool
impl Rand<bool> for bool {
    /// [0..core::u32::MAX] € N -> &0b1 [FALSE | TRUE]
    fn randup(self, method: Methods) -> bool {
        let t: u32 = u32::generate(method);
        t.get_bit(0)
    }
}
