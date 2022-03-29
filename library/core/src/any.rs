//! This module contains the `Any` trait, which enables dynamic typing
//! of any `'static` type through runtime reflection. It also contains the
//! `Provider` trait and accompanying API, which enable trait objects to provide
//! data based on typed requests, an alternate form of runtime reflection.
//!
//! # `Any` and `TypeId`
//!
//! `Any` itself can be used to get a `TypeId`, and has more features when used
//! as a trait object. As `&dyn Any` (a borrowed trait object), it has the `is`
//! and `downcast_ref` methods, to test if the contained value is of a given type,
//! and to get a reference to the inner value as a type. As `&mut dyn Any`, there
//! is also the `downcast_mut` method, for getting a mutable reference to the
//! inner value. `Box<dyn Any>` adds the `downcast` method, which attempts to
//! convert to a `Box<T>`. See the [`Box`] documentation for the full details.
//!
//! Note that `&dyn Any` is limited to testing whether a value is of a specified
//! concrete type, and cannot be used to test whether a type implements a trait.
//!
//! [`Box`]: ../../std/boxed/struct.Box.html
//!
//! # Smart pointers and `dyn Any`
//!
//! One piece of behavior to keep in mind when using `Any` as a trait object,
//! especially with types like `Box<dyn Any>` or `Arc<dyn Any>`, is that simply
//! calling `.type_id()` on the value will produce the `TypeId` of the
//! *container*, not the underlying trait object. This can be avoided by
//! converting the smart pointer into a `&dyn Any` instead, which will return
//! the object's `TypeId`. For example:
//!
//! ```
//! use std::any::{Any, TypeId};
//!
//! let boxed: Box<dyn Any> = Box::new(3_i32);
//!
//! // You're more likely to want this:
//! let actual_id = (&*boxed).type_id();
//! // ... than this:
//! let boxed_id = boxed.type_id();
//!
//! assert_eq!(actual_id, TypeId::of::<i32>());
//! assert_eq!(boxed_id, TypeId::of::<Box<dyn Any>>());
//! ```
//!
//! ## Examples
//!
//! Consider a situation where we want to log out a value passed to a function.
//! We know the value we're working on implements Debug, but we don't know its
//! concrete type. We want to give special treatment to certain types: in this
//! case printing out the length of String values prior to their value.
//! We don't know the concrete type of our value at compile time, so we need to
//! use runtime reflection instead.
//!
//! ```rust
//! use std::fmt::Debug;
//! use std::any::Any;
//!
//! // Logger function for any type that implements Debug.
//! fn log<T: Any + Debug>(value: &T) {
//!     let value_any = value as &dyn Any;
//!
//!     // Try to convert our value to a `String`. If successful, we want to
//!     // output the String`'s length as well as its value. If not, it's a
//!     // different type: just print it out unadorned.
//!     match value_any.downcast_ref::<String>() {
//!         Some(as_string) => {
//!             println!("String ({}): {}", as_string.len(), as_string);
//!         }
//!         None => {
//!             println!("{:?}", value);
//!         }
//!     }
//! }
//!
//! // This function wants to log its parameter out prior to doing work with it.
//! fn do_work<T: Any + Debug>(value: &T) {
//!     log(value);
//!     // ...do some other work
//! }
//!
//! fn main() {
//!     let my_string = "Hello World".to_string();
//!     do_work(&my_string);
//!
//!     let my_i8: i8 = 100;
//!     do_work(&my_i8);
//! }
//! ```
//!
//! # `Provider` and `Demand`
//!
//! `Provider` and the associated APIs support generic, type-driven access to data, and a mechanism
//! for implementers to provide such data. The key parts of the interface are the `Provider`
//! trait for objects which can provide data, and the [`request_value`] and [`request_ref`]
//! functions for requesting data from an object which implements `Provider`. Note that end users
//! should not call `request_*` directly, they are helper functions for intermediate implementers
//! to use to implement a user-facing interface.
//!
//! Typically, a data provider is a trait object of a trait which extends `Provider`. A user will
//! request data from the trait object by specifying the type.
//!
//! ## Data flow
//!
//! * A user requests an object, which is delegated to `request_value` or `request_ref`
//! * `request_*` creates a `Demand` object and passes it to `Provider::provide`
//! * The object provider's implementation of `Provider::provide` tries providing values of
//!   different types using `Demand::provide_*`. If the type matches the type requested by
//!   the user, it will be stored in the `Demand` object.
//! * `request_*` unpacks the `Demand` object and returns any stored value to the user.
//!
//! ## Examples
//!
//! ```
//! # #![feature(provide_any)]
//! use std::any::{Provider, Demand, request_ref};
//!
//! // Definition of MyTrait
//! trait MyTrait: Provider {
//!     // ...
//! }
//!
//! // Methods on `MyTrait` trait objects.
//! impl dyn MyTrait + '_ {
//!     /// Common case: get a reference to a field of the error.
//!     pub fn get_context_ref<T: ?Sized + 'static>(&self) -> Option<&T> {
//!         request_ref(|demand| self.provide(demand))
//!     }
//! }
//!
//! // Downstream implementation of `MyTrait` and `Provider`.
//! # struct SomeConcreteType { some_string: String }
//! impl MyTrait for SomeConcreteType {
//!     // ...
//! }
//!
//! impl Provider for SomeConcreteType {
//!     fn provide<'a>(&'a self, req: &mut Demand<'a>) {
//!         req.provide_ref::<String>(&self.some_string);
//!     }
//! }
//!
//! // Downstream usage of `MyTrait`.
//! fn use_my_trait(obj: &dyn MyTrait) {
//!     // Request a &String from obj.
//!     let _ = obj.get_context_ref::<String>().unwrap();
//! }
//! ```
//!
//! In this example, if the concrete type of `obj` in `use_my_trait` is `SomeConcreteType`, then
//! the `get_context_ref` call will return a reference to `obj.some_string`.

#![stable(feature = "rust1", since = "1.0.0")]

use crate::fmt;
use crate::intrinsics;
use crate::mem::transmute;

///////////////////////////////////////////////////////////////////////////////
// Any trait
///////////////////////////////////////////////////////////////////////////////

/// A trait to emulate dynamic typing.
///
/// Most types implement `Any`. However, any type which contains a non-`'static` reference does not.
/// See the [module-level documentation][mod] for more details.
///
/// [mod]: crate::any
// This trait is not unsafe, though we rely on the specifics of it's sole impl's
// `type_id` function in unsafe code (e.g., `downcast`). Normally, that would be
// a problem, but because the only impl of `Any` is a blanket implementation, no
// other code can implement `Any`.
//
// We could plausibly make this trait unsafe -- it would not cause breakage,
// since we control all the implementations -- but we choose not to as that's
// both not really necessary and may confuse users about the distinction of
// unsafe traits and unsafe methods (i.e., `type_id` would still be safe to call,
// but we would likely want to indicate as such in documentation).
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Any")]
pub trait Any: 'static {
    /// Gets the `TypeId` of `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::{Any, TypeId};
    ///
    /// fn is_string(s: &dyn Any) -> bool {
    ///     TypeId::of::<String>() == s.type_id()
    /// }
    ///
    /// assert_eq!(is_string(&0), false);
    /// assert_eq!(is_string(&"cookie monster".to_string()), true);
    /// ```
    #[stable(feature = "get_type_id", since = "1.34.0")]
    fn type_id(&self) -> TypeId;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: 'static + ?Sized> Any for T {
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

///////////////////////////////////////////////////////////////////////////////
// Extension methods for Any trait objects.
///////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for dyn Any {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

// Ensure that the result of e.g., joining a thread can be printed and
// hence used with `unwrap`. May eventually no longer be needed if
// dispatch works with upcasting.
#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for dyn Any + Send {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

#[stable(feature = "any_send_sync_methods", since = "1.28.0")]
impl fmt::Debug for dyn Any + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Any").finish_non_exhaustive()
    }
}

impl dyn Any {
    /// Returns `true` if the inner type is the same as `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &dyn Any) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        // Get `TypeId` of the type this function is instantiated with.
        let t = TypeId::of::<T>();

        // Get `TypeId` of the type in the trait object (`self`).
        let concrete = self.type_id();

        // Compare both `TypeId`s on equality.
        t == concrete
    }

    /// Returns some reference to the inner value if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &dyn Any) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type, and we can rely on
            // that check for memory safety because we have implemented Any for all types; no other
            // impls can exist as they would conflict with our impl.
            unsafe { Some(self.downcast_ref_unchecked()) }
        } else {
            None
        }
    }

    /// Returns some mutable reference to the inner value if it is of type `T`, or
    /// `None` if it isn't.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut dyn Any) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            // SAFETY: just checked whether we are pointing to the correct type, and we can rely on
            // that check for memory safety because we have implemented Any for all types; no other
            // impls can exist as they would conflict with our impl.
            unsafe { Some(self.downcast_mut_unchecked()) }
        } else {
            None
        }
    }

    /// Returns a reference to the inner value as type `dyn T`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_ref_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T {
        debug_assert!(self.is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &*(self as *const dyn Any as *const T) }
    }

    /// Returns a mutable reference to the inner value as type `dyn T`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_mut_unchecked::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    ///
    /// # Safety
    ///
    /// The contained value must be of type `T`. Calling this method
    /// with the incorrect type is *undefined behavior*.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_mut_unchecked<T: Any>(&mut self) -> &mut T {
        debug_assert!(self.is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &mut *(self as *mut dyn Any as *mut T) }
    }
}

impl dyn Any + Send {
    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &(dyn Any + Send)) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        <dyn Any>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &(dyn Any + Send)) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut (dyn Any + Send)) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut::<T>(self)
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_ref_unchecked::<usize>(), 1);
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// Same as the method on the type `dyn Any`.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_ref_unchecked::<T>(self) }
    }

    /// Forwards to the method defined on the type `dyn Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_mut_unchecked::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    ///
    /// # Safety
    ///
    /// Same as the method on the type `dyn Any`.
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_mut_unchecked<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_mut_unchecked::<T>(self) }
    }
}

impl dyn Any + Send + Sync {
    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn is_string(s: &(dyn Any + Send + Sync)) {
    ///     if s.is::<String>() {
    ///         println!("It's a string!");
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// is_string(&0);
    /// is_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn is<T: Any>(&self) -> bool {
        <dyn Any>::is::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn print_if_string(s: &(dyn Any + Send + Sync)) {
    ///     if let Some(string) = s.downcast_ref::<String>() {
    ///         println!("It's a string({}): '{}'", string.len(), string);
    ///     } else {
    ///         println!("Not a string...");
    ///     }
    /// }
    ///
    /// print_if_string(&0);
    /// print_if_string(&"cookie monster".to_string());
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::Any;
    ///
    /// fn modify_if_u32(s: &mut (dyn Any + Send + Sync)) {
    ///     if let Some(num) = s.downcast_mut::<u32>() {
    ///         *num = 42;
    ///     }
    /// }
    ///
    /// let mut x = 10u32;
    /// let mut s = "starlord".to_string();
    ///
    /// modify_if_u32(&mut x);
    /// modify_if_u32(&mut s);
    ///
    /// assert_eq!(x, 42);
    /// assert_eq!(&s, "starlord");
    /// ```
    #[stable(feature = "any_send_sync_methods", since = "1.28.0")]
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut::<T>(self)
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     assert_eq!(*x.downcast_ref_unchecked::<usize>(), 1);
    /// }
    /// ```
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_ref_unchecked::<T>(self) }
    }

    /// Forwards to the method defined on the type `Any`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(downcast_unchecked)]
    ///
    /// use std::any::Any;
    ///
    /// let mut x: Box<dyn Any> = Box::new(1_usize);
    ///
    /// unsafe {
    ///     *x.downcast_mut_unchecked::<usize>() += 1;
    /// }
    ///
    /// assert_eq!(*x.downcast_ref::<usize>().unwrap(), 2);
    /// ```
    #[unstable(feature = "downcast_unchecked", issue = "90850")]
    #[inline]
    pub unsafe fn downcast_mut_unchecked<T: Any>(&mut self) -> &mut T {
        // SAFETY: guaranteed by caller
        unsafe { <dyn Any>::downcast_mut_unchecked::<T>(self) }
    }
}

///////////////////////////////////////////////////////////////////////////////
// TypeID and its methods
///////////////////////////////////////////////////////////////////////////////

/// A `TypeId` represents a globally unique identifier for a type.
///
/// Each `TypeId` is an opaque object which does not allow inspection of what's
/// inside but does allow basic operations such as cloning, comparison,
/// printing, and showing.
///
/// A `TypeId` is currently only available for types which ascribe to `'static`,
/// but this limitation may be removed in the future.
///
/// While `TypeId` implements `Hash`, `PartialOrd`, and `Ord`, it is worth
/// noting that the hashes and ordering will vary between Rust releases. Beware
/// of relying on them inside of your code!
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct TypeId {
    t: u64,
}

impl TypeId {
    /// Returns the `TypeId` of the type this generic function has been
    /// instantiated with.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::any::{Any, TypeId};
    ///
    /// fn is_string<T: ?Sized + Any>(_s: &T) -> bool {
    ///     TypeId::of::<String>() == TypeId::of::<T>()
    /// }
    ///
    /// assert_eq!(is_string(&0), false);
    /// assert_eq!(is_string(&"cookie monster".to_string()), true);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_type_id", issue = "77125")]
    pub const fn of<T: ?Sized + 'static>() -> TypeId {
        TypeId { t: intrinsics::type_id::<T>() }
    }
}

/// Returns the name of a type as a string slice.
///
/// # Note
///
/// This is intended for diagnostic use. The exact contents and format of the
/// string returned are not specified, other than being a best-effort
/// description of the type. For example, amongst the strings
/// that `type_name::<Option<String>>()` might return are `"Option<String>"` and
/// `"std::option::Option<std::string::String>"`.
///
/// The returned string must not be considered to be a unique identifier of a
/// type as multiple types may map to the same type name. Similarly, there is no
/// guarantee that all parts of a type will appear in the returned string: for
/// example, lifetime specifiers are currently not included. In addition, the
/// output may change between versions of the compiler.
///
/// The current implementation uses the same infrastructure as compiler
/// diagnostics and debuginfo, but this is not guaranteed.
///
/// # Examples
///
/// ```rust
/// assert_eq!(
///     std::any::type_name::<Option<String>>(),
///     "core::option::Option<alloc::string::String>",
/// );
/// ```
#[must_use]
#[stable(feature = "type_name", since = "1.38.0")]
#[rustc_const_unstable(feature = "const_type_name", issue = "63084")]
pub const fn type_name<T: ?Sized>() -> &'static str {
    intrinsics::type_name::<T>()
}

/// Returns the name of the type of the pointed-to value as a string slice.
/// This is the same as `type_name::<T>()`, but can be used where the type of a
/// variable is not easily available.
///
/// # Note
///
/// This is intended for diagnostic use. The exact contents and format of the
/// string are not specified, other than being a best-effort description of the
/// type. For example, `type_name_of_val::<Option<String>>(None)` could return
/// `"Option<String>"` or `"std::option::Option<std::string::String>"`, but not
/// `"foobar"`. In addition, the output may change between versions of the
/// compiler.
///
/// This function does not resolve trait objects,
/// meaning that `type_name_of_val(&7u32 as &dyn Debug)`
/// may return `"dyn Debug"`, but not `"u32"`.
///
/// The type name should not be considered a unique identifier of a type;
/// multiple types may share the same type name.
///
/// The current implementation uses the same infrastructure as compiler
/// diagnostics and debuginfo, but this is not guaranteed.
///
/// # Examples
///
/// Prints the default integer and float types.
///
/// ```rust
/// #![feature(type_name_of_val)]
/// use std::any::type_name_of_val;
///
/// let x = 1;
/// println!("{}", type_name_of_val(&x));
/// let y = 1.0;
/// println!("{}", type_name_of_val(&y));
/// ```
#[must_use]
#[unstable(feature = "type_name_of_val", issue = "66359")]
#[rustc_const_unstable(feature = "const_type_name", issue = "63084")]
pub const fn type_name_of_val<T: ?Sized>(_val: &T) -> &'static str {
    type_name::<T>()
}

///////////////////////////////////////////////////////////////////////////////
// Provider trait
///////////////////////////////////////////////////////////////////////////////

/// Trait implemented by a type which can dynamically provide values based on type.
#[unstable(feature = "provide_any", issue = "none")]
pub trait Provider {
    /// Object providers should implement this method to provide *all* values they are able to
    /// provide using `req`.
    #[unstable(feature = "provide_any", issue = "none")]
    fn provide<'a>(&'a self, req: &mut Demand<'a>);
}

/// Request a value from the `Provider`.
#[unstable(feature = "provide_any", issue = "none")]
pub fn request_value<'a, T, F>(f: F) -> Option<T>
where
    T: 'static,
    F: FnOnce(&mut Demand<'a>),
{
    request_by_type_tag::<'a, tags::Value<T>, _>(f)
}

/// Request a reference from the `Provider`.
#[unstable(feature = "provide_any", issue = "none")]
pub fn request_ref<'a, T, F>(f: F) -> Option<&'a T>
where
    T: ?Sized + 'static,
    F: FnOnce(&mut Demand<'a>),
{
    request_by_type_tag::<'a, tags::Ref<tags::MaybeSizedValue<T>>, _>(f)
}

/// Request a specific value by tag from the `Provider`.
fn request_by_type_tag<'a, I, F>(f: F) -> Option<I::Reified>
where
    I: tags::Type<'a>,
    F: FnOnce(&mut Demand<'a>),
{
    let mut tagged = TaggedOption::<'a, I>(None);
    f(tagged.as_demand());
    tagged.0
}

///////////////////////////////////////////////////////////////////////////////
// Demand and its methods
///////////////////////////////////////////////////////////////////////////////

/// A helper object for providing objects by type.
///
/// An object provider provides values by calling this type's provide methods.
#[allow(missing_debug_implementations)]
#[unstable(feature = "provide_any", issue = "none")]
// SAFETY: `TaggedOption::as_demand` relies on this precise definition.
#[repr(transparent)]
pub struct Demand<'a>(dyn Erased<'a> + 'a);

impl<'a> Demand<'a> {
    /// Provide a value or other type with only static lifetimes.
    #[unstable(feature = "provide_any", issue = "none")]
    pub fn provide_value<T, F>(&mut self, fulfil: F) -> &mut Self
    where
        T: 'static,
        F: FnOnce() -> T,
    {
        self.provide_with::<tags::Value<T>, F>(fulfil)
    }

    /// Provide a reference, note that the referee type must be bounded by `'static`, but may be unsized.
    #[unstable(feature = "provide_any", issue = "none")]
    pub fn provide_ref<T: ?Sized + 'static>(&mut self, value: &'a T) -> &mut Self {
        self.provide::<tags::Ref<tags::MaybeSizedValue<T>>>(value)
    }

    /// Provide a value with the given `Type` tag.
    fn provide<I>(&mut self, value: I::Reified) -> &mut Self
    where
        I: tags::Type<'a>,
    {
        if let Some(res @ TaggedOption(None)) = self.0.downcast_mut::<I>() {
            res.0 = Some(value);
        }
        self
    }

    /// Provide a value with the given `Type` tag, using a closure to prevent unnecessary work.
    fn provide_with<I, F>(&mut self, fulfil: F) -> &mut Self
    where
        I: tags::Type<'a>,
        F: FnOnce() -> I::Reified,
    {
        if let Some(res @ TaggedOption(None)) = self.0.downcast_mut::<I>() {
            res.0 = Some(fulfil());
        }
        self
    }
}

///////////////////////////////////////////////////////////////////////////////
// Type tags
///////////////////////////////////////////////////////////////////////////////

mod tags {
    //! Type tags are used to identify a type using a separate value. This module includes type tags
    //! for some very common types.
    //!
    //! Many users of the provider APIs will not need to use type tags at all. But if you want to
    //! use them with more complex types (typically those including lifetime parameters), you will
    //! need to write your own tags.

    use crate::marker::PhantomData;

    /// This trait is implemented by specific tag types in order to allow
    /// describing a type which can be requested for a given lifetime `'a`.
    ///
    /// A few example implementations for type-driven tags can be found in this
    /// module, although crates may also implement their own tags for more
    /// complex types with internal lifetimes.
    pub trait Type<'a>: Sized + 'static {
        /// The type of values which may be tagged by this tag for the given
        /// lifetime.
        type Reified: 'a;
    }

    /// Similar to the [`Type`] trait, but represents a type which may be unsized (i.e., has a
    /// `'Sized` bound). E.g., `str`.
    pub trait MaybeSizedType<'a>: Sized + 'static {
        type Reified: 'a + ?Sized;
    }

    impl<'a, T: Type<'a>> MaybeSizedType<'a> for T {
        type Reified = T::Reified;
    }

    /// Type-based tag for types bounded by `'static`, i.e., with no borrowed element.
    #[derive(Debug)]
    pub struct Value<T: 'static>(PhantomData<T>);

    impl<'a, T: 'static> Type<'a> for Value<T> {
        type Reified = T;
    }

    /// Type-based tag similar to [`Value`] but which may be unsized (i.e., has a `'Sized` bound).
    #[derive(Debug)]
    pub struct MaybeSizedValue<T: ?Sized + 'static>(PhantomData<T>);

    impl<'a, T: ?Sized + 'static> MaybeSizedType<'a> for MaybeSizedValue<T> {
        type Reified = T;
    }

    /// Type-based tag for `&'a T` types.
    #[derive(Debug)]
    pub struct Ref<I>(PhantomData<I>);

    impl<'a, I: MaybeSizedType<'a>> Type<'a> for Ref<I> {
        type Reified = &'a I::Reified;
    }
}

/// An `Option` with a type tag `I`.
///
/// Since this struct implements `Erased`, the type can be erased to make a dynamically typed
/// option. The type can be checked dynamically using `Erased::tag_id` and since this is statically
/// checked for the concrete type, there is some degree of type safety.
#[repr(transparent)]
struct TaggedOption<'a, I: tags::Type<'a>>(Option<I::Reified>);

impl<'a, I: tags::Type<'a>> TaggedOption<'a, I> {
    fn as_demand(&mut self) -> &mut Demand<'a> {
        // SAFETY: transmuting `&mut (dyn Erased<'a> + 'a)` to `&mut Demand<'a>` is safe since
        // `Demand` is repr(transparent) and holds only a `dyn Erased<'a> + 'a`.
        unsafe { transmute(self as &mut (dyn Erased<'a> + 'a)) }
    }
}

/// Represents a type-erased but identifiable object.
///
/// This trait is exclusively implemented by the `TaggedOption` type.
trait Erased<'a>: 'a {
    /// The `TypeId` of the erased type.
    fn tag_id(&self) -> TypeId;
}

impl<'a, I: tags::Type<'a>> Erased<'a> for TaggedOption<'a, I> {
    fn tag_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
}

#[unstable(feature = "provide_any", issue = "none")]
impl<'a> dyn Erased<'a> {
    /// Returns some reference to the dynamic value if it is tagged with `I`,
    /// or `None` if it isn't.
    #[inline]
    fn downcast_mut<I>(&mut self) -> Option<&mut TaggedOption<'a, I>>
    where
        I: tags::Type<'a>,
    {
        if self.tag_id() == TypeId::of::<I>() {
            // SAFETY: Just checked whether we're pointing to an I.
            Some(unsafe { &mut *(self as *mut Self as *mut TaggedOption<'a, I>) })
        } else {
            None
        }
    }
}
