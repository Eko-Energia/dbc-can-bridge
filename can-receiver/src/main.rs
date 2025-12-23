mod setup;
mod integration;
mod app;

use color_eyre::eyre::Result;
use setup::config;
use app::App;

fn main() -> Result<()> {
    color_eyre::install()?;
    // Initialize configuration
    config::init_config()?;

    // Run the app
    App::new()?.run()
}