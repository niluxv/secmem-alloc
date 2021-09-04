use secmem_alloc::zeroizing_alloc::ZeroizeAlloc;
use std::alloc::{Layout, System};

#[global_allocator]
static GLOBAL: ZeroizeAlloc<System> = ZeroizeAlloc::new(System);

#[test]
fn box_9b() {
    let boxed = Box::new([1_u8; 9]);
    // drop `boxed`
}

#[test]
fn vec_grow_shrink() {
    let mut vec = vec![1_u8; 109];
    vec.extend(std::iter::repeat(37).take(141));
    vec.shrink_to_fit();
    vec.truncate(17);
    vec.shrink_to_fit();
    // drop `vec`
}

#[test]
fn alloc_zeroed() {
    let layout = Layout::new::<[u8; 16]>();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    for i in 0..16 {
        let val: u8 = unsafe { ptr.add(i).read() };
        assert_eq!(val, 0_u8);
    }
    unsafe {
        std::alloc::dealloc(ptr, layout);
    }
}
