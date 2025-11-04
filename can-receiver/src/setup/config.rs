use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    io::Write,
};
use waveshare_usb_can_a::CanBaudRate;
use color_eyre::eyre::{Result, eyre};

const CONFIG_FILE_NAME: &str = "config.txt";

#[derive(Debug, Clone)]
pub struct Config {
    pub device_port: String,
    pub can_baud_rate: CanBaudRate,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_port: get_default_device_port(),
            can_baud_rate: CanBaudRate::R500kBd,
        }
    }
}

impl Config {
    /// Loads configuration from file or creates default one
    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        
        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            let config = Self::default();
            config.save_to_file(&config_path)?;
            println!("Created default configuration file: {}", config_path.display());
            Ok(config)
        }
    }

    /// Loads configuration from file
    fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config = Self::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "device_port" => {
                        config.device_port = value.to_string();
                    }
                    "can_baud_rate" => {
                        config.can_baud_rate = parse_can_baud_rate(value)?;
                    }
                    _ => {
                        println!("Unknown configuration key: {}", key);
                    }
                }
            }
        }

        Ok(config)
    }

    /// Saves configuration to file
    fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let content = format!(
            "# CAN Receiver Configuration\n\
             # Path to CAN device\n\
             device_port={}\n\
             \n\
             # CAN transmission speed (5k, 10k, 20k, 50k, 100k, 125k, 200k, 250k, 400k, 500k, 800k, 1000k)\n\
             can_baud_rate={}\n",
            self.device_port,
            format_can_baud_rate(self.can_baud_rate)
        );

        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}

/// Returns default device path depending on operating system
fn get_default_device_port() -> String {
    #[cfg(target_os = "linux")]
    {
        "/dev/ttyUSB0".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "COM4".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "/dev/cu.wchusbserial110".to_string()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "/dev/ttyUSB0".to_string() // Fallback for other systems
    }
}

/// Returns path to configuration file (in the same directory as binary file)
fn get_config_path() -> Result<PathBuf> {
    let mut exe_path = std::env::current_exe()?;
    exe_path.pop(); // Removes binary file name, leaving directory
    exe_path.push(CONFIG_FILE_NAME);
    Ok(exe_path)
}

/// Parses string to CanBaudRate
fn parse_can_baud_rate(value: &str) -> Result<CanBaudRate> {
    match value.to_lowercase().as_str() {
        "5k" | "5000" => Ok(CanBaudRate::R5kBd),
        "10k" | "10000" => Ok(CanBaudRate::R10kBd),
        "20k" | "20000" => Ok(CanBaudRate::R20kBd),
        "50k" | "50000" => Ok(CanBaudRate::R50kBd),
        "100k" | "100000" => Ok(CanBaudRate::R100kBd),
        "125k" | "125000" => Ok(CanBaudRate::R125kBd),
        "200k" | "200000" => Ok(CanBaudRate::R200kBd),
        "250k" | "250000" => Ok(CanBaudRate::R250kBd),
        "400k" | "400000" => Ok(CanBaudRate::R400kBd),
        "500k" | "500000" => Ok(CanBaudRate::R500kBd),
        "800k" | "800000" => Ok(CanBaudRate::R800kBd),
        "1000k" | "1000000" | "1m" => Ok(CanBaudRate::R1000kBd),
        _ => Err(eyre!("Unknown CAN speed: {}. Available: 5k, 10k, 20k, 50k, 100k, 125k, 200k, 250k, 400k, 500k, 800k, 1000k", value)),
    }
}

/// Formats CanBaudRate to string
fn format_can_baud_rate(rate: CanBaudRate) -> &'static str {
    match rate {
        CanBaudRate::R5kBd => "5k",
        CanBaudRate::R10kBd => "10k",
        CanBaudRate::R20kBd => "20k",
        CanBaudRate::R50kBd => "50k",
        CanBaudRate::R100kBd => "100k",
        CanBaudRate::R125kBd => "125k",
        CanBaudRate::R200kBd => "200k",
        CanBaudRate::R250kBd => "250k",
        CanBaudRate::R400kBd => "400k",
        CanBaudRate::R500kBd => "500k",
        CanBaudRate::R800kBd => "800k",
        CanBaudRate::R1000kBd => "1000k",
    }
}

/// Global singleton for storing configuration in memory
static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

/// Initializes configuration (call once at program start)
pub fn init_config() -> Result<()> {
    let config = Config::load()?;
    CONFIG.set(Mutex::new(config))
        .map_err(|_| eyre!("Configuration has already been initialized"))?;

    Ok(())
}

/// Returns device path
pub fn get_device_port() -> Result<String> {
    let config = CONFIG.get()
        .ok_or_else(|| eyre!("Configuration not initialized. Call init_config() first."))?
        .lock()
        .map_err(|_| eyre!("Configuration access error"))?;
    
    Ok(config.device_port.clone())
}

/// Returns CAN speed
pub fn get_can_baud_rate() -> Result<CanBaudRate> {
    let config = CONFIG.get()
        .ok_or_else(|| eyre!("Configuration not initialized. Call init_config() first."))?
        .lock()
        .map_err(|_| eyre!("Configuration access error"))?;
    
    Ok(config.can_baud_rate)
}