// u8 - u64, i8 - i64, f32, f64, usize, isize
mod native_basic;

// &'static T, &'static mut T
mod native_ref;

// str, &'static str
mod native_str;

// ()  (T1,)  (T1, T2)  ...  (T1, T2, .. T12) // 'static str
mod native_tuple;

// [T], [T; N]
mod native_array;
