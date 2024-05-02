use super::*;

fn test_b127_zeroizer<Z: MemZeroizer>(z: Z) {
    let mut array: [u8; 127] = [0xAF; 127];
    let ptr: *mut u8 = (&mut array[..]).as_mut_ptr();
    unsafe {
        z.zeroize_mem(ptr, 127);
    }
    assert_eq!(array, [0u8; 127]);
}

fn test_b239_lowalign_zeroizer<Z: MemZeroizer>(z: Z) {
    // ensure we get 8 byte aligned memory
    let mut array: [u64; 30] = [0x_AFAFAFAF_AFAFAFAF; 30];

    // zeroize everything but the first byte, so the pointer to the memory will have
    // an alignment of 1 byte

    let array_ptr: *mut u64 = (&mut array[..]).as_mut_ptr();
    // 1 byte aligned; SAFETY: resulting `ptr` still pointing in array
    let ptr: *mut u8 = unsafe { array_ptr.cast::<u8>().add(1) };
    // this should still be safe
    unsafe { z.zeroize_mem(ptr, 30 * 8 - 1) };

    let mut expected: [u64; 30] = [0; 30];
    expected[0] = u64::from_ne_bytes([0x_AF, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&array[..], &expected[..]);
}

#[cfg(feature = "nightly_core_intrinsics")]
#[test]
fn test_b127_volatile_memset_zeroizer() {
    test_b127_zeroizer(VolatileMemsetZeroizer);
}

#[test]
fn test_b127_volatile_write8_zeroizer() {
    test_b127_zeroizer(VolatileWrite8Zeroizer);
}

#[cfg(feature = "nightly_core_intrinsics")]
#[test]
fn test_b239_lowalign_volatile_memset_zeroizer() {
    test_b239_lowalign_zeroizer(VolatileMemsetZeroizer);
}

#[test]
fn test_b239_lowalign_volatile_write8_zeroizer() {
    test_b239_lowalign_zeroizer(VolatileWrite8Zeroizer);
}
