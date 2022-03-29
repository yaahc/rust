// run-pass
#![feature(provide_any)]
use std::any::{Provider, Demand, request_ref, request_value};

// Definition of MyTrait
trait MyTrait: Provider {
    // ...
}

// Methods on `MyTrait` trait objects.
impl dyn MyTrait + '_ {
    /// Common case: get a reference to a field of the error.
    pub fn get_context_ref<T: ?Sized + 'static>(&self) -> Option<&T> {
        request_ref(|demand| self.provide(demand))
    }

    /// Common case: get a reference to a field of the error.
    pub fn get_context<T: 'static>(&self) -> Option<T> {
        request_value(|demand| self.provide(demand))
    }
}

struct IsFatal;

struct SomeFatalError;

impl MyTrait for SomeFatalError {}

impl Provider for SomeFatalError {
    fn provide<'a>(&'a self, req: &mut Demand<'a>) {
        req.provide_ref(&IsFatal).provide_value(|| IsFatal);
    }
}

struct SomeNonFatalError;

impl MyTrait for SomeNonFatalError {}

impl Provider for SomeNonFatalError {
    fn provide<'a>(&'a self, _req: &mut Demand<'a>) { }
}

fn is_fatal(obj: &dyn MyTrait) -> bool {
    obj.get_context_ref::<IsFatal>().is_some() && obj.get_context::<IsFatal>().is_some()
}

fn main() {
    assert!(is_fatal(&SomeFatalError));
    assert!(!is_fatal(&SomeNonFatalError));
}
