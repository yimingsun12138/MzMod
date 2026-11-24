#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    if let Err(error) = mzmod_controller::ui::run() {
        rfd::MessageDialog::new()
            .set_title("MzMod")
            .set_level(rfd::MessageLevel::Error)
            .set_description(format!("{error:#}"))
            .show();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("MzMod Controller runs on Windows.");
}
