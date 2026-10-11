// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Before any real work, so the allocator cache does not keep the scan's pages.
    wrasse_core::platform::prefer_small_heap();
    wrasse_app_lib::run()
}
