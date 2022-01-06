// run-pass
#![feature(panic_error)]

use std::panic::PanicInfo;
use std::fmt;

fn install_panic_hook() {
    let old_hook = std::panic::take_hook();
    let new_hook = move |info: &PanicInfo| {
        old_hook(info);
        if let Some(source) = info.error() {
            eprintln!("Error: {}", source);
        }
    };
    std::panic::set_hook(Box::new(new_hook));
}

fn main() {
    install_panic_hook();
    let error = MyError;
    core::panic::panic_error(error);
}

#[derive(Debug)]
struct MyError;

impl core::error::Error for MyError {}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "my error occurred")
    }
}
