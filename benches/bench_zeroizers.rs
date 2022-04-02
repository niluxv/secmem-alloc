use criterion::{criterion_group, criterion_main, Criterion};
#[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
use secmem_alloc::zeroize::AsmRepStosZeroizer;
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
use secmem_alloc::zeroize::X86_64AvxZeroizer;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use secmem_alloc::zeroize::X86_64Sse2Zeroizer;
use secmem_alloc::zeroize::{
    LibcZeroizer, MemZeroizer, VolatileMemsetZeroizer, VolatileWrite8Zeroizer,
    VolatileWriteZeroizer,
};

fn zeroize_b127<Z: MemZeroizer>(z: Z, array: &mut [u8; 127]) {
    unsafe {
        let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
        z.zeroize_mem(ptr, 127);
    }
}

fn zeroize_b128<Z: MemZeroizer>(z: Z, array: &mut [u8; 128]) {
    unsafe {
        let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
        z.zeroize_mem(ptr, 128);
    }
}

fn zeroize_b1024<Z: MemZeroizer>(z: Z, array: &mut [u8; 1024]) {
    unsafe {
        let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
        z.zeroize_mem(ptr, 1024);
    }
}

macro_rules! bench_zeroizers {
    ($cgroup:ident, $bench_function:ident, $array:ident) => {
        $cgroup.bench_function("VolatileMemsetZeroizer", |b| {
            b.iter(|| $bench_function(VolatileMemsetZeroizer, &mut $array.0))
        });
        $cgroup.bench_function("LibcZeroizer", |b| {
            b.iter(|| $bench_function(LibcZeroizer, &mut $array.0))
        });
        $cgroup.bench_function("VolatileWriteZeroizer", |b| {
            b.iter(|| $bench_function(VolatileWriteZeroizer, &mut $array.0))
        });
        $cgroup.bench_function("VolatileWrite8Zeroizer", |b| {
            b.iter(|| $bench_function(VolatileWrite8Zeroizer, &mut $array.0))
        });
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            $cgroup.bench_function("X86_64Sse2Zeroizer", |b| {
                b.iter(|| $bench_function(X86_64Sse2Zeroizer, &mut $array.0))
            });
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
        {
            $cgroup.bench_function("X86_64AvxZeroizer", |b| {
                b.iter(|| $bench_function(X86_64AvxZeroizer, &mut $array.0))
            });
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "ermsb"))]
        {
            $cgroup.bench_function("AsmRepStosZeroizer", |b| {
                b.iter(|| $bench_function(AsmRepStosZeroizer, &mut $array.0))
            });
        }
    };
}

#[repr(align(32))]
struct Align32<const N: usize>([u8; N]);

fn zeroize_byte127(c: &mut Criterion) {
    let mut array: Align32<127> = Align32([0xAF; 127]);
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 127 byte zeroize");
    bench_zeroizers!(cgroup, zeroize_b127, array);
}

fn zeroize_byte128(c: &mut Criterion) {
    let mut array: Align32<128> = Align32([0xAF; 128]);
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 128 byte zeroize");
    bench_zeroizers!(cgroup, zeroize_b128, array);
}

fn zeroize_byte1024(c: &mut Criterion) {
    let mut array: Align32<1024> = Align32([0xAF; 1024]);
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 1024 byte zeroize");
    bench_zeroizers!(cgroup, zeroize_b1024, array);
}

criterion_group!(
    bench_zeroize_bytes,
    zeroize_byte127,
    zeroize_byte128,
    zeroize_byte1024
);
criterion_main!(bench_zeroize_bytes);
