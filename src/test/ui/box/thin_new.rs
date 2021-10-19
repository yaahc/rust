#![feature(thin_box)]
// run-pass
use std::boxed::ThinBox;
use std::error::Error;
use std::fmt;

fn main() {
    let _a: ThinBox<dyn Error> = ThinBox::new(Foo);
}

#[derive(Debug)]
struct Foo;

impl fmt::Display for Foo {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl Error for Foo {}
