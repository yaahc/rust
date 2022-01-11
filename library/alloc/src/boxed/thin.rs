// Based on
// https://github.com/matthieu-m/rfc2580/blob/b58d1d3cba0d4b5e859d3617ea2d0943aaa31329/examples/thin.rs
// by matthieu-m
use crate::alloc::{self, Layout};
use core::cmp;
use core::fmt::{self, Debug, Formatter};
use core::marker::{PhantomData, Unsize};
use core::mem;
use core::ops::{Deref, DerefMut};
use core::ptr::Pointee;
use core::ptr::{self, NonNull};

/// ThinBox.
///
/// A thin pointer, regardless of T.
#[unstable(feature = "thin_box", issue = "none")]
pub struct ThinBox<T: ?Sized + Pointee> {
    ptr: WithHeader<T::Metadata>,
    _marker: PhantomData<fn(T) -> T>,
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: Pointee> ThinBox<T> {
    /// Moves a type to the heap with it's `Metadata` stored in the heap allocation instead of on
    /// the stack.
    pub fn new(value: T) -> Self {
        let dyn_ref: &T = &value;
        let (_, meta) = (dyn_ref as *const T).to_raw_parts();
        let ptr = WithHeader::new(meta, value).expect("No allocation failure");
        ThinBox { ptr, _marker: PhantomData }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<Dyn: ?Sized + Pointee> ThinBox<Dyn> {
    /// Moves a type to the heap with it's `Metadata` stored in the heap allocation instead of on
    /// the stack.
    pub fn new_unsize<T>(value: T) -> Self
    where
        T: Unsize<Dyn>,
    {
        let dyn_ref: &Dyn = &value;
        let (_, meta) = (dyn_ref as *const Dyn).to_raw_parts();
        let ptr = WithHeader::new(meta, value).expect("No allocation failure");
        ThinBox { ptr, _marker: PhantomData }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: ?Sized + Debug + Pointee> Debug for ThinBox<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self)
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: ?Sized + Pointee> Deref for ThinBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let value = self.data();
        let metadata = self.meta();
        let pointer = ptr::from_raw_parts(value as *const (), metadata);
        unsafe { &*pointer }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: ?Sized + Pointee> DerefMut for ThinBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        let value = self.data();
        let metadata = self.meta();
        let pointer = ptr::from_raw_parts::<T>(value as *const (), metadata) as *mut T;
        unsafe { &mut *pointer }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: ?Sized + Pointee> Drop for ThinBox<T> {
    fn drop(&mut self) {
        unsafe {
            let value: &mut T = &mut *self;
            let value: *mut T = value as *mut T;
            self.ptr.drop::<T>(value);
        }
    }
}

#[unstable(feature = "thin_box", issue = "none")]
impl<T: ?Sized + Pointee> ThinBox<T> {
    fn meta(&self) -> T::Metadata {
        //  Safety:
        //  -   NonNull and valid.
        unsafe { *self.ptr.header() }
    }

    fn data(&self) -> *mut u8 {
        self.ptr.value()
    }
}

struct WithHeader<H>(*mut u8, PhantomData<H>);

impl<H> WithHeader<H> {
    fn new<T>(header: H, value: T) -> Option<WithHeader<H>> {
        let layout = Self::alloc_layout(mem::size_of::<T>(), mem::align_of::<T>());
        let aligned_header_size = Self::aligned_header_size(layout.align());

        unsafe {
            let ptr = NonNull::new(alloc::alloc(layout))?;

            //  Safety:
            //  -   The size is at least `aligned_header_size`.
            let ptr = ptr.as_ptr().offset(aligned_header_size as isize) as *mut _;

            let result = WithHeader(ptr, PhantomData);
            ptr::write(result.header(), header);
            ptr::write(result.value().cast(), value);

            Some(result)
        }
    }

    //  Safety:
    //  -   Assumes that `value` can be dereferenced.
    unsafe fn drop<T: ?Sized>(&self, value: *mut T) {
        unsafe {
            let layout = Self::alloc_layout(mem::size_of_val(&*value), mem::align_of_val(&*value));
            let aligned_header_size = Self::aligned_header_size(layout.align());

            ptr::drop_in_place::<T>(value as *mut T);
            alloc::dealloc(self.0.offset(-(aligned_header_size as isize)), layout);
        }
    }

    fn header(&self) -> *mut H {
        //  Safety:
        //  -   At least `size_of::<H>()` bytes are allocated ahead of the pointer.
        unsafe { self.0.offset(-(Self::header_size() as isize)) as *mut H }
    }

    fn value(&self) -> *mut u8 {
        self.0
    }

    fn header_size() -> usize {
        mem::size_of::<H>()
    }

    fn alloc_layout(value_size: usize, value_align: usize) -> Layout {
        assert!(Self::header_size() <= usize::MAX / 2 + 1);

        let alloc_align = cmp::max(mem::align_of::<H>(), value_align);
        let alloc_size = Self::aligned_header_size(alloc_align) + value_size;

        //  Safety:
        //  -   `align` is not zero and a power of two, as guaranteed by `mem::align_of`.
        //  -   The size can be rounded up to `align` without overflow, or `H` is way too big.
        unsafe { Layout::from_size_align_unchecked(alloc_size, alloc_align) }
    }

    fn aligned_header_size(alloc_align: usize) -> usize {
        cmp::max(Self::header_size(), alloc_align)
    }
}
