use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub struct ProgressSpinner {
    spinner: ProgressBar,
}

impl ProgressSpinner {
    pub fn run(message: String, animation_time: u64) -> Self {
        let spinner = ProgressBar::new_spinner();

        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["   ", ".  ", ".. ", "...", " ..", "  ."])
                .template("{msg}{spinner}")
                .unwrap(),
        );

        spinner.set_message(message);
        spinner.enable_steady_tick(Duration::from_millis(animation_time));

        ProgressSpinner { spinner }
    }

    pub fn finish(&self, message: String) {
        self.spinner.finish_with_message(message);
    }
}
