use core::mem;

use alloc::boxed::ThinBox;

#[test]
fn want_niche_optimization() {
    assert_eq!(
        mem::size_of::<Option<ThinBox<[i32]>>>(),
        mem::size_of::<ThinBox<[i32]>>(),
    );
}

fn want_thin() {
    assert_eq!(
        mem::size_of::<ThinBox<[i32]>>(),
        mem::size_of::<*mut ()>(),
    );

    trait Tr {}

    assert_eq!(
        mem::size_of::<ThinBox<dyn Tr>>(),
        mem::size_of::<*mut ()>(),
    );
}
