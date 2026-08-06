#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![cfg_attr(target_os = "windows", allow(unsafe_op_in_unsafe_fn))]

mod geometry;

#[cfg(target_os = "windows")]
mod win;

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = win::run() {
        win::show_fatal_error(&error.to_string());
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("Rustpture is supported on Windows only.");
}
