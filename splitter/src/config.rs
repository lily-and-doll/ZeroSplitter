use std::{
	fs::{File, read_to_string},
	io::{Read, Write},
	sync::OnceLock,
};

use eframe::egui::{
	Context, Id, Modal, ModalResponse, Response, RichText, Separator, TextEdit, Ui, ViewportBuilder, ViewportId,
};
use toml::{Table, Value};

use crate::{ZeroError, theme::GREEN};

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
		decoration_button: match table.get("decoration_button") {
			Some(Value::Boolean(b)) => *b,
			_ => false,
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
	pub decoration_button: bool,
}

pub fn options_menu(ctx: &Context, db: &crate::database::Database, open: &mut bool) -> () {
	ctx.show_viewport_immediate(
		ViewportId::from_hash_of("options_menu_viewport"),
		ViewportBuilder::default().with_title("Options"),
		|ctx, _| {
			eframe::egui::CentralPanel::default().show(ctx, |ui| {
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
