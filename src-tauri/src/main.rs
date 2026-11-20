#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    #[cfg(target_os = "linux")]
    reponiq_lib::run_git_supervisor();
    reponiq_lib::run()
}
