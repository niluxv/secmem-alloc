use criterion::{criterion_group, criterion_main, Criterion};
use secmem_alloc::zeroize::{
    AsmRepStosZeroizer, LibcZeroizer, MemZeroizer, VolatileMemsetZeroizer, VolatileWrite8Zeroizer,
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

fn zeroize_byte127(c: &mut Criterion) {
    let mut array: [u8; 127] = [0xAF; 127];
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 127 byte zeroize");
    cgroup.bench_function("VolatileMemsetZeroizer", |b| {
        b.iter(|| zeroize_b127(VolatileMemsetZeroizer, &mut array))
    });
    cgroup.bench_function("LibcZeroizer", |b| {
        b.iter(|| zeroize_b127(LibcZeroizer, &mut array))
    });
    cgroup.bench_function("AsmRepStosZeroizer", |b| {
        b.iter(|| zeroize_b127(AsmRepStosZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWriteZeroizer", |b| {
        b.iter(|| zeroize_b127(VolatileWriteZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWrite8Zeroizer", |b| {
        b.iter(|| zeroize_b127(VolatileWrite8Zeroizer, &mut array))
    });
}

fn zeroize_byte128(c: &mut Criterion) {
    let mut array: [u8; 128] = [0xAF; 128];
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 128 byte zeroize");
    cgroup.bench_function("VolatileMemsetZeroizer", |b| {
        b.iter(|| zeroize_b128(VolatileMemsetZeroizer, &mut array))
    });
    cgroup.bench_function("LibcZeroizer", |b| {
        b.iter(|| zeroize_b128(LibcZeroizer, &mut array))
    });
    cgroup.bench_function("AsmRepStosZeroizer", |b| {
        b.iter(|| zeroize_b128(AsmRepStosZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWriteZeroizer", |b| {
        b.iter(|| zeroize_b128(VolatileWriteZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWrite8Zeroizer", |b| {
        b.iter(|| zeroize_b128(VolatileWrite8Zeroizer, &mut array))
    });
}

fn zeroize_byte1024(c: &mut Criterion) {
    let mut array: [u8; 1024] = [0xAF; 1024];
    let mut cgroup = c.benchmark_group("MemZeroizer::zeroize_mem 1024 byte zeroize");
    cgroup.bench_function("VolatileMemsetZeroizer", |b| {
        b.iter(|| zeroize_b1024(VolatileMemsetZeroizer, &mut array))
    });
    cgroup.bench_function("LibcZeroizer", |b| {
        b.iter(|| zeroize_b1024(LibcZeroizer, &mut array))
    });
    cgroup.bench_function("AsmRepStosZeroizer", |b| {
        b.iter(|| zeroize_b1024(AsmRepStosZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWriteZeroizer", |b| {
        b.iter(|| zeroize_b1024(VolatileWriteZeroizer, &mut array))
    });
    cgroup.bench_function("VolatileWrite8Zeroizer", |b| {
        b.iter(|| zeroize_b1024(VolatileWrite8Zeroizer, &mut array))
    });
}

criterion_group!(
    bench_zeroize_bytes,
    zeroize_byte127,
    zeroize_byte128,
    zeroize_byte1024
);
criterion_main!(bench_zeroize_bytes);
