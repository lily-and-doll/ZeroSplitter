use std::{
	fs::{File, read_to_string},
	io::{Read, Write},
	sync::OnceLock,
};

use toml::{Table, Value};

use crate::ZeroError;

pub static CONFIG: OnceLock<Config> = OnceLock::new();

const CONFIG_PATH: &'static str = "config.toml";

pub fn load_config() -> Result<(), ZeroError> {
	let config_str = match read_to_string(CONFIG_PATH) {
		Ok(s) => s,
		Err(e) => match e.kind() {
			std::io::ErrorKind::NotFound => create_config()?,
			_ => return Err(ZeroError::IOError(e)),
		},
	};
	let table = config_str.parse::<Table>().map_err(ZeroError::TOMLError)?;

	let config = Config {
		zoom_level: match table.get("zoom_level") {
			Some(Value::Float(f)) => *f as f32,
			_ => 1.0,
		},
	};

	CONFIG.set(config).map_err(|_| ZeroError::StaticAlreadyInit)?;
	Ok(())
}

fn create_config() -> Result<String, ZeroError> {
	let mut file = File::create_new(CONFIG_PATH).map_err(ZeroError::IOError)?;
	file.write_all(include_bytes!("../assets/default_config.toml"))
		.map_err(ZeroError::IOError)?;
	let mut ret = String::new();
	file.read_to_string(&mut ret).map_err(ZeroError::IOError)?;

	Ok(ret)
}

pub struct Config {
	pub zoom_level: f32,
}
