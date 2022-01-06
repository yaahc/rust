use crate::any::Any;
use crate::fmt;
use crate::panic::Location;
#[cfg(not(bootstrap))]
use crate::error::Error;

/// A struct providing information about a panic.
///
/// `PanicInfo` structure is passed to a panic hook set by the [`set_hook`]
/// function.
///
/// [`set_hook`]: ../../std/panic/fn.set_hook.html
///
/// # Examples
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|panic_info| {
///     if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
///         println!("panic occurred: {:?}", s);
///     } else {
///         println!("panic occurred");
///     }
/// }));
///
/// panic!("Normal panic");
/// ```
#[lang = "panic_info"]
#[stable(feature = "panic_hooks", since = "1.10.0")]
#[derive(Debug)]
pub struct PanicInfo<'a> {
    payload: PayloadKind<'a>,
    message: Option<&'a fmt::Arguments<'a>>,
    location: &'a Location<'a>,
}

#[derive(Debug)]
enum PayloadKind<'a> {
    Any(&'a (dyn Any + Send)),
    #[cfg(not(bootstrap))]
    Error(&'a (dyn Error + 'static)),
}

impl<'a> PayloadKind<'a> {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        match self {
            PayloadKind::Any(payload) => payload.downcast_ref(),
            #[cfg(not(bootstrap))]
            PayloadKind::Error(_) => None,
            // PayloadKind::Error(error) => error.downcast_ref(),
        }
    }
}

impl<'a> PanicInfo<'a> {
    #[unstable(
        feature = "panic_internals",
        reason = "internal details of the implementation of the `panic!` and related macros",
        issue = "none"
    )]
    #[doc(hidden)]
    #[inline]
    pub fn internal_constructor(
        message: Option<&'a fmt::Arguments<'a>>,
        location: &'a Location<'a>,
    ) -> Self {
        struct NoPayload;
        PanicInfo { location, message, payload: PayloadKind::Any(&NoPayload) }
    }

    #[unstable(
        feature = "panic_internals",
        reason = "internal details of the implementation of the `panic!` and related macros",
        issue = "none"
    )]
    #[doc(hidden)]
    #[inline]
    #[cfg(not(bootstrap))]
    pub fn error_constructor(
        // message: Option<&'a fmt::Arguments<'a>>,
        error: &'a (dyn crate::error::Error + 'static),
        location: &'a Location<'a>,
    ) -> Self {
        PanicInfo { location, message: None, payload: PayloadKind::Error(error) }
    }

    #[unstable(
        feature = "panic_internals",
        reason = "internal details of the implementation of the `panic!` and related macros",
        issue = "none"
    )]
    #[doc(hidden)]
    #[inline]
    pub fn set_payload(&mut self, info: &'a (dyn Any + Send)) {
        self.payload = PayloadKind::Any(info);
    }

    /// Returns the payload associated with the panic.
    ///
    /// This will commonly, but not always, be a `&'static str` or [`String`].
    ///
    /// [`String`]: ../../std/string/struct.String.html
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
    ///         println!("panic occurred: {:?}", s);
    ///     } else {
    ///         println!("panic occurred");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn payload(&self) -> &(dyn Any + Send) {
        #[cfg(not(bootstrap))]
        struct HackerVoice;
        match self.payload {
            PayloadKind::Any(payload) => payload,
            #[cfg(not(bootstrap))]
            PayloadKind::Error(_) => &HackerVoice, // I'm in
        }
    }

    /// If the `panic!` macro from the `core` crate (not from `std`)
    /// was used with a formatting string and some additional arguments,
    /// returns that message ready to be used for example with [`fmt::write`]
    #[must_use]
    #[unstable(feature = "panic_info_message", issue = "66745")]
    pub fn message(&self) -> Option<&fmt::Arguments<'_>> {
        self.message
    }

    /// If the panic was created with `panic_error` returns the source error of the panic.
    #[must_use]
    #[unstable(feature = "panic_error", issue = "none")]
    #[cfg(not(bootstrap))]
    pub fn error(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self.payload {
            PayloadKind::Any(_) => None,
            PayloadKind::Error(source) => Some(source),
        }
    }

    /// Returns information about the location from which the panic originated,
    /// if available.
    ///
    /// This method will currently always return [`Some`], but this may change
    /// in future versions.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}' at line {}",
    ///             location.file(),
    ///             location.line(),
    ///         );
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn location(&self) -> Option<&Location<'_>> {
        // NOTE: If this is changed to sometimes return None,
        // deal with that case in std::panicking::default_hook and core::panicking::panic_fmt.
        Some(&self.location)
    }
}

#[stable(feature = "panic_hook_display", since = "1.26.0")]
impl fmt::Display for PanicInfo<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("panicked at ")?;
        if let Some(message) = self.message {
            write!(formatter, "'{}', ", message)?
        } else if let Some(payload) = self.payload.downcast_ref::<&'static str>() {
            write!(formatter, "'{}', ", payload)?
        }
        // NOTE: we cannot use downcast_ref::<String>() here
        // since String is not available in libcore!
        // The payload is a String when `std::panic!` is called with multiple arguments,
        // but in that case the message is also available.

        self.location.fmt(formatter)
    }
}
