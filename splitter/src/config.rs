use std::{
	fs::{File, OpenOptions, read_to_string},
	io::{Read, Write},
	sync::OnceLock,
};

use crate::VERSION;

use eframe::egui::{
	Context, Id, RichText, Separator, TextEdit, ViewportBuilder, ViewportId,
};
use toml::{Table, Value};

use crate::{ZeroError, theme::GREEN, update::check_for_updates};

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
	let table = config_str.parse::<Table>()?;

	let mut writer = OpenOptions::new().append(true).open(CONFIG_PATH)?;

	let config = Config {
		zoom_level: match table.get("zoom_level") {
			Some(Value::Float(f)) => *f as f32,
			None => {
				writer.write_all(include_bytes!("../assets/config_sections/zoom_level.toml"))?;
				1.0
			}
			_ => return Err(ZeroError::ConfigError("zoom_level".to_owned())),
		},
		decoration_button: match table.get("decoration_button") {
			Some(Value::Boolean(b)) => *b,
			None => {
				writer.write_all(include_bytes!("../assets/config_sections/decoration_button.toml"))?;
				false
			}
			_ => return Err(ZeroError::ConfigError("decoration_button".to_owned())),
		},
		check_for_updates: match table.get("check_for_updates") {
			Some(Value::Boolean(b)) => *b,
			None => {
				writer.write_all(include_bytes!("../assets/config_sections/check_for_updates.toml"))?;
				true
			}
			_ => return Err(ZeroError::ConfigError("check_for_updates".to_owned())),
		},
	};

	CONFIG.set(config).map_err(|_| ZeroError::StaticAlreadyInit)?;
	Ok(())
}

fn create_config() -> Result<String, ZeroError> {
	let mut file = File::create_new(CONFIG_PATH)?;
	file.write_all(include_bytes!("../assets/config_sections/heading.toml"))?;
	let mut ret = String::new();
	file.read_to_string(&mut ret)?;

	Ok(ret)
}

pub struct Config {
	pub zoom_level: f32,
	pub decoration_button: bool,
	pub check_for_updates: bool,
}

pub fn options_menu(ctx: &Context, db: &crate::database::Database, open: &mut bool) -> () {
	ctx.show_viewport_immediate(
		ViewportId::from_hash_of("options_menu_viewport"),
		ViewportBuilder::default().with_title("Options"),
		|ctx, _| {
			eframe::egui::CentralPanel::default().show(ctx, |ui| {
				if CONFIG.get().unwrap().check_for_updates {
					// UPDATER
					ui.horizontal(|ui| {
						ui.label(RichText::new("Updater").color(GREEN).heading());
						ui.add(Separator::default().horizontal())
					});
					ui.horizontal(|ui| {
						let update_label_id = Id::new("update_label");
						if ui.button("Check for updates").clicked() {
							ctx.data_mut(|data| {
								data.insert_temp(
									update_label_id,
									match check_for_updates() {
										Ok(Some(url)) => format!("Update avaliable - {url}"),
										Ok(None) => format!("Up to date - {VERSION}"),
										Err(e) => format!("Failed to check for update - {e:?}"),
									},
								)
							});
						}
						ui.label(
							ctx.data(|data| data.get_temp::<String>(update_label_id))
								.unwrap_or_default(),
						);
					});
				};
				// IMPORTER
				ui.horizontal(|ui| {
					ui.label(RichText::new("Importer").color(GREEN).heading());
					ui.add(Separator::default().horizontal())
				});
				// Category name
				let category_name_id = ui.label("Category name").id;
				let mut category_name = ctx
					.data(|data| data.get_temp::<String>(category_name_id))
					.unwrap_or_default();
				ui.add(TextEdit::singleline(&mut category_name));
				ctx.data_mut(|data| data.insert_temp(category_name_id, category_name.clone()));

				// Split data
				let splits_data_id = ui.label("Split scores").id;
				let mut run_string = ctx
					.data(|data| data.get_temp::<String>(splits_data_id))
					.unwrap_or_default();
				ui.add(TextEdit::multiline(&mut run_string).hint_text("111, 222, 333, 444, 555, 666, 777, 888"));
				ctx.data_mut(|data| data.insert_temp(splits_data_id, run_string.clone()));

				if ui.button("IMPORT").clicked() {
					let splits = run_string
						.split(", ")
						.map(|s| s.parse().unwrap_or_default())
						.collect::<Vec<i32>>();
					match db.import_run(splits.clone(), &category_name) {
						Ok(_) => {
							println!(
								"Successfully imported run with {} splits and total score {}",
								splits.len(),
								splits.iter().sum::<i32>()
							);
							run_string.clear();
						}
						Err(err) => println!("Import failed: {err}"),
					};
				};
			});

			if ctx.input(|i| i.viewport().close_requested()) {
				*open = false
			};
		},
	);
}
