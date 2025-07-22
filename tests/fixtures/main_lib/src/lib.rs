use crate::sub::my_sub_lib_fn;
use dependency_lib::my_lib_fn as dep_lib_fn;

mod sub;

pub fn my_lib_fn(left: u64, right: u64) -> u64 {
    let accu = left + right;
    let accu = accu + dep_lib_fn(left, right);
    let accu = accu + my_sub_lib_fn() + private_fn();
    accu
}

/// A private function that returns a constant value.
fn private_fn() -> u64 {
    42
}
