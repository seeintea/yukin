// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    yukin_lib::run_crash_monitor_if_requested();
    yukin_lib::run()
}
