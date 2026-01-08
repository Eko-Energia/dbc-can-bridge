mod setup;
mod integration;
mod app;

use std::fs::File;

use color_eyre::eyre::{Result, eyre};
use log::LevelFilter;
use setup::config;
use app::App;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger};

extern crate simplelog;
#[macro_use] extern crate log;

fn main() -> Result<()> {
    color_eyre::install()?;

    let log_config = ConfigBuilder::new()
        .set_time_offset_to_local()
        .map_err(|_| eyre!("Failed to get local time offset"))?
        .build();

    CombinedLogger::init(
        vec![
        TermLogger::new(LevelFilter::Trace, log_config.clone(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(LevelFilter::Trace, log_config, File::create("can-receiver.log")?),
    ]
    )?;

    let result = (|| -> Result<_> {
        // Initialize configuration
        config::init_config()?;
        // Run the app
        App::new()?.run()
    })();

    match result {
        Err(e) => {
            error!("App error: {:?}", e);
            // fix for double error messages
            Ok(())
        }
        Ok(s) => Ok(s)
    }
}