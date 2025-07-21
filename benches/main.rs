use criterion::{criterion_group, criterion_main, Criterion};
use cw_storage_plus::IntKey;
use rand::Rng;
use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256PlusPlus};
use std::hint::black_box;
use std::time::Duration;

fn bench_signed_int_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("Signed int keys");

    #[inline(always)]
    fn k() -> i32 {
        Xoshiro256PlusPlus::seed_from_u64(42).random_range(i32::MIN..i32::MAX)
    }
    // Save a value for future asserts.
    let k_check = k();

    type Buf = [u8; size_of::<i32>()];

    group.bench_function("i32 to_cw_bytes: xored (u32) + to_be_bytes", |b| {
        #[inline(always)]
        fn to_cw_bytes(value: &i32) -> Buf {
            (*value as u32 ^ i32::MIN as u32).to_be_bytes()
        }

        assert_eq!(to_cw_bytes(&0), i32::to_cw_bytes(&0));
        assert_eq!(to_cw_bytes(&k_check), i32::to_cw_bytes(&k_check));
        assert_eq!(
            to_cw_bytes(&k_check.wrapping_neg()),
            i32::to_cw_bytes(&k_check.wrapping_neg())
        );

        b.iter(|| {
            let k = k();
            black_box(to_cw_bytes(&k));
            black_box(to_cw_bytes(&k.wrapping_neg()));
        });
    });

    group.bench_function("i32 to_cw_bytes: xored (u128) + to_be_bytes", |b| {
        #[inline(always)]
        fn to_cw_bytes(value: &i32) -> Buf {
            ((*value as u128 ^ i32::MIN as u128) as i32).to_be_bytes()
        }

        assert_eq!(to_cw_bytes(&0), i32::to_cw_bytes(&0));
        assert_eq!(to_cw_bytes(&k_check), i32::to_cw_bytes(&k_check));
        assert_eq!(
            to_cw_bytes(&k_check.wrapping_neg()),
            i32::to_cw_bytes(&k_check.wrapping_neg())
        );

        b.iter(|| {
            let k = k();
            black_box(to_cw_bytes(&k));
            black_box(to_cw_bytes(&k.wrapping_neg()));
        });
    });

    group.bench_function("i32 to_cw_bytes: mut to_be_bytes + xor", |b| {
        #[inline(always)]
        fn to_cw_bytes(value: &i32) -> Buf {
            let mut buf = i32::to_be_bytes(*value);
            buf[0] ^= 0x80;
            buf
        }

        assert_eq!(to_cw_bytes(&0), i32::to_cw_bytes(&0));
        assert_eq!(to_cw_bytes(&k_check), i32::to_cw_bytes(&k_check));
        assert_eq!(
            to_cw_bytes(&k_check.wrapping_neg()),
            i32::to_cw_bytes(&k_check.wrapping_neg())
        );

        b.iter(|| {
            let k = k();
            black_box(to_cw_bytes(&k));
            black_box(to_cw_bytes(&k.wrapping_neg()));
        });
    });

    group.bench_function("i32 to_cw_bytes: branching plus / minus", |b| {
        #[inline(always)]
        fn to_cw_bytes(value: &i32) -> Buf {
            if value >= &0i32 {
                (*value as u32).wrapping_sub(i32::MIN as u32).to_be_bytes()
            } else {
                (*value as u32).wrapping_add(i32::MIN as u32).to_be_bytes()
            }
        }

        assert_eq!(to_cw_bytes(&0), i32::to_cw_bytes(&0));
        assert_eq!(to_cw_bytes(&k_check), i32::to_cw_bytes(&k_check));
        assert_eq!(
            to_cw_bytes(&k_check.wrapping_neg()),
            i32::to_cw_bytes(&k_check.wrapping_neg())
        );

        b.iter(|| {
            let k = k();
            black_box(to_cw_bytes(&k));
            black_box(to_cw_bytes(&k.wrapping_neg()));
        });
    });

    group.finish();
}

fn bench_unsigned_int_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("Unsigned int keys");

    #[inline(always)]
    fn k() -> u32 {
        Xoshiro256PlusPlus::seed_from_u64(42).random_range(u32::MIN..u32::MAX)
    }
    // Save a value for future asserts.
    let k_check = k();

    type Buf = [u8; size_of::<u32>()];

    group.bench_function("u32 to_cw_bytes", |b| {
        #[inline(always)]
        fn to_cw_bytes(value: &u32) -> Buf {
            value.to_be_bytes()
        }

        assert_eq!(to_cw_bytes(&0), u32::to_cw_bytes(&0));
        assert_eq!(to_cw_bytes(&k_check), u32::to_cw_bytes(&k_check));

        b.iter(|| {
            let k = k();
            // Run twice for comparability with other benchmarks.
            black_box(to_cw_bytes(&k));
            black_box(to_cw_bytes(&k));
        });
    });

    group.finish();
}

fn make_config() -> Criterion {
    Criterion::default()
        .without_plots()
        .measurement_time(Duration::new(5, 0))
        .sample_size(10)
        .configure_from_args()
}

criterion_group!(
    name = signed_int_key;
    config = make_config();
    targets = bench_signed_int_key
);

criterion_group!(
    name = unsigned_int_key;
    config = make_config();
    targets = bench_unsigned_int_key
);

criterion_main!(signed_int_key, unsigned_int_key);
