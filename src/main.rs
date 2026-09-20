// RustVoxel client (Windows / OpenGL via hand-written FFI).
#![windows_subsystem = "windows"]

#[cfg(any(windows, feature = "fficheck"))]
fn main() {
    if let Err(e) = rustvoxel::app::run() {
        #[cfg(windows)]
        rustvoxel::win32::fatal(&e);
        #[cfg(not(windows))]
        {
            eprintln!("fatal: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(any(windows, feature = "fficheck")))]
fn main() {
    eprintln!("RustVoxel client: build and run on Windows (cargo build --release).");
}
