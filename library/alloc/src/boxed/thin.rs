use crate::alloc::{alloc, Layout};
use core::fmt;
use core::marker::Unsize;
use core::ptr::{self, Pointee};

extern "C" {
    /// A dummy type used to force `ThinBox` to be unsized while not requiring references to it to
    /// be wide pointers.
    type ErasedExtern;
}

#[repr(C)]
struct WithLayout<T: ?Sized, Dyn: ?Sized> {
    meta: <Dyn as Pointee>::Metadata,
    value: T,
}

type Thin<Dyn> = WithLayout<ErasedExtern, Dyn>;

/// A pointer type for heap allocation of trait objects which is guaranteed to take up only one
/// usize on the stack.
#[unstable(feature = "thin_box", issue = "none")]
pub struct ThinBox<Dyn: ?Sized>(*mut Thin<Dyn>);

impl<Dyn: ?Sized> ThinBox<Dyn> {
    /// Moves a type to the heap with it's `Metadata` stored in the heap allocation instead of on
    /// the stack.
    #[unstable(feature = "thin_box", issue = "none")]
    pub fn new<T>(x: T) -> Self
    where
        T: Unsize<Dyn>,
    {
        let u_ref: &Dyn = &x;
        let (_, metadata) = (u_ref as *const Dyn).to_raw_parts();
        let layout = Layout::new::<WithLayout<T, Dyn>>();
        let ptr = unsafe { alloc(layout) } as *mut WithLayout<T, Dyn>;
        unsafe {
            ptr::addr_of_mut!((*ptr).meta).write(metadata);
            ptr::addr_of_mut!((*ptr).value).write(x);
        }
        let ptr = ptr as *mut Thin<Dyn>;
        ThinBox(ptr)
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<Dyn: ?Sized> AsRef<Dyn> for ThinBox<Dyn> {
    fn as_ref(&self) -> &Dyn {
        let inner = unsafe { &*self.0 as &Thin<Dyn> };
        let metadata = inner.meta;
        let value = &inner.value as *const _ as *const ();
        let ptr = ptr::from_raw_parts(value, metadata);
        unsafe { &*ptr as &Dyn }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<Dyn: fmt::Debug> fmt::Debug for ThinBox<Dyn> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.as_ref();
        fmt::Debug::fmt(inner, f)
    }
}
