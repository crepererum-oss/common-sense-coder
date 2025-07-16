use crate::sub::my_sub_lib_fn;
use dependency_lib::my_lib_fn as dep_lib_fn;

mod sub;

pub fn my_lib_fn(left: u64, right: u64) -> u64 {
    left + right + dep_lib_fn(left, right) + my_sub_lib_fn()
}
