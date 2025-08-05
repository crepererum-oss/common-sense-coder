use crate::sub::my_sub_lib_fn;
use dependency_lib::my_lib_fn as dep_lib_fn;
use workspace_member::my_lib_fn as workspace_member_lib_fn;

mod sub;

/// Calculate a few things.
///
/// ```
/// use main_lib::my_lib_fn;
///
/// my_lib_fn(1, 2);
/// ```
pub fn my_lib_fn(left: u64, right: u64) -> u64 {
    let accu = left + right;
    let accu = accu + dep_lib_fn(left, right);
    let accu = accu + workspace_member_lib_fn();
    let accu = accu + my_sub_lib_fn() + my_private_lib_fn() + foo();
    accu
}

/// A private function that returns a constant value.
fn my_private_lib_fn() -> u64 {
    42
}

/// Another private function that returns a constant value.
fn foo() -> u64 {
    42
}

fn main() {
    println!("Hello, world!");
}

/// A struct that "shadows" the `main` function.
///
/// See <https://github.com/rust-lang/rust-analyzer/issues/19486#issuecomment-2817393342>.
pub(crate) struct MyMainStruct {
    pub field: u64,
}
