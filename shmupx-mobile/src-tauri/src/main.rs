// Sem console extra no Windows (o Lite também roda no desktop para desenvolver).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shmupx_mobile_lib::run()
}
