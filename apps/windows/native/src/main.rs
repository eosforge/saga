//! Saga Windows native desktop entrypoint.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Launch Saga Windows native desktop application
    native::run();
}
