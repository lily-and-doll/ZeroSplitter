use std::{
	env,
	fs::{self, File},
	net::UdpSocket,
	sync::{
		OnceLock,
		mpsc::{self, Receiver, Sender},
	},
	thread,
	time::Duration,
};

use common::FrameData;
use eframe::{
	App, Frame, NativeOptions,
	egui::{
		Align, CentralPanel, Color32, ComboBox, Context, IconData, Id, Key, Layout, Sides, ThemePreference,
		TopBottomPanel, ViewportBuilder, ViewportId,
	},
};
use log::{error, warn};
use serde::{Deserialize, Serialize};

mod hook;
mod system;
mod ui;

const DARK_GREEN: Color32 = Color32::from_rgb(0, 0x4f, 0x4d);
const GREEN: Color32 = Color32::from_rgb(0, 0x94, 0x79);
const LIGHT_ORANGE: Color32 = Color32::from_rgb(0xff, 0xc0, 0x73);
const DARK_ORANGE: Color32 = Color32::from_rgb(0xff, 0x80, 0);

static EGUI_CTX: OnceLock<Context> = OnceLock::new();

fn main() {
	pretty_env_logger::init();

	let options = NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size([300., 290.])
			.with_icon(IconData::default())
			.with_title("ZeroSplitter"),
		..Default::default()
	};

	let (tx, rx) = mpsc::channel();

	thread::spawn(|| ipc_thread(tx));

	eframe::run_native(
		"ZeroSplitter",
		options,
		Box::new(|c| {
			let _ = EGUI_CTX.set(c.egui_ctx.clone());
			c.egui_ctx.set_theme(ThemePreference::Dark);
			Ok(Box::new(ZeroSplitter::load(rx)))
		}),
	)
	.unwrap();
}

fn ipc_thread(channel: Sender<FrameData>) {
	let socket = UdpSocket::bind("127.0.0.1:23888").expect("Binding socket");
	socket
		.set_read_timeout(Some(Duration::from_secs(1)))
		.expect("Setting socket timeout");

	let mut buf = [0; size_of::<FrameData>()];
	loop {
		while socket.recv(&mut buf).is_ok() {
			let data = FrameData::from_bytes(buf);
			let _ = channel.send(data);
			if let Some(ctx) = EGUI_CTX.get() {
				ctx.request_repaint();
			}
		}
		// timed out, hook the game
		hook::hook_zeroranger();
	}
}

struct ZeroSplitter {
	categories: Vec<Category>,
	current_category: usize,
	data_source: Receiver<FrameData>,
	last_frame: FrameData,
	current_run: Run,
	current_split: Option<usize>,
	current_split_score_offset: i32,
	waiting_for_category: bool,
	waiting_for_rename: bool,
	waiting_for_confirm: bool,
	dialog_rx: Receiver<Option<EntryDialogData>>,
	dialog_tx: Sender<Option<EntryDialogData>>,
	comparison: Category,
	active: bool,
}

impl ZeroSplitter {
	fn new(data_source: Receiver<FrameData>) -> Self {
		let (tx, rx) = mpsc::channel();
		let mut default_categories = Vec::new();
		default_categories.push(Category::new("Type-C GO".to_string(), Gamemode::GreenOrange));
		default_categories.push(Category::new("Type-B GO".to_string(), Gamemode::GreenOrange));
		default_categories.push(Category::new("Type-C WV".to_string(), Gamemode::WhiteVanilla));
		default_categories.push(Category::new("Type-B WV".to_string(), Gamemode::WhiteVanilla));
		Self {
			categories: default_categories,
			data_source,
			last_frame: Default::default(),
			current_category: 0,
			current_run: Run::new(Gamemode::GreenOrange),
			current_split: None,
			current_split_score_offset: 0,
			dialog_rx: rx,
			dialog_tx: tx,
			waiting_for_category: false,
			waiting_for_rename: false,
			waiting_for_confirm: false,
			comparison: Category::new("<null>".to_string(), Gamemode::GreenOrange),
			active: false,
		}
	}

	fn load(data_source: Receiver<FrameData>) -> Self {
		let data_path = env::current_exe()
			.expect("Could not get program directory")
			.with_file_name("zs_data.json");

		match fs::exists(&data_path) {
			Ok(true) => (),
			Ok(false) => return Self::new(data_source),
			Err(e) => {
				warn!("Could not tell if data file exists: {}", e);
				return Self::new(data_source);
			}
		}

		match File::open(&data_path) {
			Ok(file) => {
				let try_new_cat = serde_json::from_reader::<_, Vec<Category>>(&file);
				if let Ok(data) = try_new_cat {
					if data.is_empty() {
						Self::new(data_source)
					} else {
						Self {
							current_category: 0,
							categories: data,
							..Self::new(data_source)
						}
					}
				} else {
					// An attempt at old-save migration. Probably doesn't work.
					let try_old_cat = serde_json::from_reader::<_, Vec<OldCategory>>(&file);
					if let Ok(data) = try_old_cat {
						Self {
							current_category: 0,
							categories: data.iter().map(|c| c.to_new(Gamemode::GreenOrange)).collect(),
							..Self::new(data_source)
						}
					} else {
						panic!(
							"Data failed to parse as Category or OldCategory at {:?}: {} and {}",
							&data_path,
							try_new_cat.unwrap_err(),
							try_old_cat.unwrap_err()
						)
					}
				}
			}
			Err(e) => panic!("Could not open extant data file at {:?}: {}", &data_path, e),
		}
	}

	fn save_splits(&mut self) {
		self.categories[self.current_category].update_from_run(&self.current_run);

		let data_path = env::current_exe()
			.expect("Could not get program directory")
			.with_file_name("zs_data.json");
		let file = match File::create(&data_path) {
			Ok(file) => file,
			Err(err) => {
				error!("Could not save: Could not open data file {:?}: {}", &data_path, err);
				return;
			}
		};

		if let Err(err) = serde_json::to_writer_pretty(file, &self.categories) {
			error!("Error writing save: {}", err);
		}
	}

	fn update_frame(&mut self, frame: FrameData) {
		// Difficulty is ZR-speak for gamemode
		if frame.difficulty == 0 {
			self.update_greenorange(frame);
		} else if frame.difficulty == -1 {
			self.update_whitevanilla(frame);
		} else if frame.difficulty == 1 {
			// Black Onion placeholder
		}
		self.last_frame = frame;
	}

	fn update_greenorange(&mut self, frame: FrameData) {
		// Skip update if current category isn't Green Orange
		if self.categories[self.current_category].mode != Gamemode::GreenOrange {
			return;
		}

		// Reset if we just left the menu or returned to 1-1
		if frame.stage != self.last_frame.stage && (self.last_frame.is_menu() || frame.is_first_stage()) {
			self.reset();
		}

		if !frame.is_menu() {
			let frame_split = (frame.stage - 1 - frame.game_loop) as usize;

			if frame_split >= 8 {
				// TLB or credits
				return;
			}

			// Split if necessary
			if frame.stage != self.last_frame.stage {
				self.current_split = Some(frame_split);
				self.current_split_score_offset = self.last_frame.total_score();
				self.save_splits();
			}

			// If our score got reset by a continue, fix the score offset.
			if self.current_split_score_offset > frame.total_score() {
				self.current_split_score_offset = 0;
			}

			// Update run and split scores
			self.current_run.score = frame.total_score();
			let split_score = frame.total_score() - self.current_split_score_offset;
			self.current_run.splits[frame_split] = split_score;
		} else {
			// End the run if we're back on the menu
			self.end_run();
		}
	}

	fn update_whitevanilla(&mut self, frame: FrameData) {
		// Skip update if current category isn't White Vanilla
		if self.categories[self.current_category].mode != Gamemode::WhiteVanilla {
			return;
		}

		// Reset if we returned to 1-1
		if frame.total_score() == 0 && self.last_frame.total_score() > 0 || self.last_frame.is_menu() {
			// Only support starting at stage 1 for now
			if frame.is_first_stage() {
				self.reset();
				self.current_split = Some(0);
				return
			} // TODO: detect start of each other stage and start splits there properly
		}

		if !frame.is_menu() && self.active {
			// Split if necessary; score requirement prevents spurious splits after a reset
			if frame.timer_wave == 0 && self.last_frame.timer_wave != 0 && frame.total_score() > 0 {
				self.current_split = self.current_split.or(Some(0)).map(|s| s + 1);
				self.current_split_score_offset = self.last_frame.total_score();
				self.save_splits();
			}

			// TODO: reimplement continue support

			// Update run and split scores
			self.current_run.score = frame.total_score();
			let split_score = frame.total_score() - self.current_split_score_offset;
			self.current_run.splits[self.current_split.unwrap_or(0)] = split_score;
		} else {
			// End the run if we're back on the menu
			self.end_run();
		}
	}

	fn reset(&mut self) {
		self.end_run();
		self.current_run = Run::new(self.categories[self.current_category].mode);
		self.comparison = self.categories[self.current_category].clone();
		self.current_split = Some(0);
		self.active = true;
		self.current_split_score_offset = 0;
	}

	fn end_run(&mut self) {
		self.save_splits();
		self.current_split = None;
		self.active = false;
	}
}

impl App for ZeroSplitter {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		while let Ok(data) = self.data_source.try_recv() {
			self.update_frame(data);
		}

		CentralPanel::default().show(ctx, |ui| {
			ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
				ui.horizontal(|ui| {
					ui.label("Category: ");
					ComboBox::from_label("").show_index(ui, &mut self.current_category, self.categories.len(), |i| {
						&self.categories[i].name
					});
					if ui.small_button("+").clicked() {
						self.waiting_for_category = true;
					}
					/*if ui.button("Delete").clicked() {
						self.waiting_for_confirm = true;
					}*/
					if ui.button("Rename").clicked() {
						self.waiting_for_rename = true;
					}
				});

				// Detect gamemode change persist between frames
				let prev_mode_id = Id::new("prev_mode");
				let cur_mode = self.categories[self.current_category].mode;
				if let Some(prev_mode) = ctx.data(|data| data.get_temp::<Gamemode>(prev_mode_id)) {
					if prev_mode != cur_mode {
						self.reset();
					}
				}
				ctx.data_mut(|data| data.insert_temp(prev_mode_id, cur_mode));

				let cur_category = &self.categories[self.current_category];

				match cur_mode {
					Gamemode::GreenOrange => {
						for (i, split) in self.current_run.splits.iter().enumerate() {
							let stage_n = (i & 3) + 1;
							let loop_n = (i >> 2) + 1;
							let best = cur_category.best_splits[i];
							let stored_best = self.comparison.personal_best.splits[i];
							let pb = self.comparison.personal_best.splits[i];

							Sides::new().show(
								ui,
								|left| {
									left.label(format!("{}-{}", loop_n, stage_n));

									if best > 0 {
										left.colored_label(GREEN, best.to_string());
									}
								},
								|right| {
									if *split != 0 || self.current_split == Some(i) {
										let split_color = if self.current_split == Some(i) {
											Color32::WHITE
										} else {
											DARK_ORANGE
										};

										right.colored_label(split_color, split.to_string());

										if self.current_split != Some(i) {
											// past split, we should show a diff
											let diff = *split - pb;
											let diff_color = if *split > stored_best {
												LIGHT_ORANGE
											} else if diff >= 0 {
												Color32::WHITE
											} else {
												DARK_GREEN
											};

											if diff > 0 {
												right.colored_label(diff_color, format!("+{}", diff));
											} else {
												right.colored_label(diff_color, diff.to_string());
											}
										}
									} else {
										right.colored_label(DARK_GREEN, "--");
									}
								},
							);
						}
					}
					Gamemode::WhiteVanilla => {
						for (i, &split) in self.current_run.splits.iter().enumerate() {
							let best_this_checkpoint = cur_category.best_splits[i];
							let stored_best = self.comparison.personal_best.splits[i];
							let pb = self.comparison.personal_best.splits[i];

							Sides::new().show(
								ui,
								|left| {
									left.label(vanilla_split_names(i));

									if best_this_checkpoint > 0 {
										left.colored_label(GREEN, best_this_checkpoint.to_string());
									}
								},
								|right| {
									if split != 0 || self.current_split == Some(i) {
										let split_color = if self.current_split == Some(i) {
											Color32::WHITE
										} else {
											DARK_ORANGE
										};

										right.colored_label(split_color, split.to_string());

										if self.current_split != Some(i) {
											// past split, we should show a diff
											let diff = split - pb;
											let diff_color = if split > stored_best {
												LIGHT_ORANGE
											} else if diff >= 0 {
												Color32::WHITE
											} else {
												DARK_GREEN
											};

											if diff > 0 {
												right.colored_label(diff_color, format!("+{}", diff));
											} else {
												right.colored_label(diff_color, diff.to_string());
											}
										}
									} else {
										right.colored_label(DARK_GREEN, "--");
									}
								},
							);
						}
					}
					Gamemode::BlackOnion => {
						todo!()
					}
				}

				ui.label(format!("Personal Best: {}", cur_category.personal_best.score));
				ui.label(format!("Sum of Best: {}", cur_category.best_splits.iter().sum::<i32>()))
			});
		});

		if self.waiting_for_category {
			if let Ok(new_category) = self.dialog_rx.try_recv() {
				if let Some(data) = new_category {
					self.categories.push(Category::new(data.textbox, data.mode));
					self.current_category = self.categories.len() - 1;
					self.save_splits();
				}
				self.waiting_for_category = false;
			} else {
				category_maker_dialog(ctx, self.dialog_tx.clone(), "Enter new category name", true);
			}
		}

		if self.waiting_for_rename {
			if let Ok(new_category) = self.dialog_rx.try_recv() {
				if let Some(data) = new_category {
					self.categories[self.current_category].name = data.textbox;
					self.categories[self.current_category].mode = data.mode;
					self.save_splits();
				}
				self.waiting_for_rename = false;
			} else {
				category_maker_dialog(ctx, self.dialog_tx.clone(), "Enter new name for category", false);
			}
		}

		if self.waiting_for_confirm {
			if let Ok(Some(confirmation)) = self.dialog_rx.try_recv() {
				if confirmation.textbox == "Deleted" {
					self.categories.remove(self.current_category);
					self.current_category = self.current_category.saturating_sub(1);
				}
				self.waiting_for_confirm = false;
			} else {
				confirm_dialog(
					ctx,
					self.dialog_tx.clone(),
					format!(
						"Are you sure you want to delete category {}?",
						self.categories[self.current_category].name
					),
				);
			}
		}
	}

	fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
		self.save_splits();
	}
}

fn entry_dialog(ctx: &Context, tx: Sender<String>, msg: &'static str) {
	let vp_builder = ViewportBuilder::default()
		.with_title("ZeroSplitter")
		.with_active(true)
		.with_resizable(false)
		.with_minimize_button(false)
		.with_maximize_button(false)
		.with_inner_size([200., 100.]);

	ctx.show_viewport_deferred(ViewportId::from_hash_of("entry dialog"), vp_builder, move |ctx, _| {
		if ctx.input(|input| input.viewport().close_requested()) {
			let _ = tx.send("".to_string());
			request_repaint();
			return;
		}

		let text_id = Id::new("edit text");
		let mut edit_str = ctx.data_mut(|data| data.get_temp_mut_or_insert_with(text_id, || String::new()).clone());

		CentralPanel::default().show(ctx, |ui| {
			ui.vertical_centered_justified(|ui| {
				ui.label(msg);
				if ui.text_edit_singleline(&mut edit_str).lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
					let _ = tx.send(edit_str.clone());
					request_repaint();
				}
			});
		});

		ctx.data_mut(|data| {
			data.insert_temp(text_id, edit_str);
		});
	});
}

/// Category entry dialoge menu with a gamemode dropdown.
fn category_maker_dialog(ctx: &Context, tx: Sender<Option<EntryDialogData>>, msg: &'static str, mode_select: bool) {
	let vp_builder = ViewportBuilder::default()
		.with_title("ZeroSplitter")
		.with_active(true)
		.with_resizable(false)
		.with_minimize_button(false)
		.with_maximize_button(false)
		.with_inner_size([200., 100.]);

	ctx.show_viewport_deferred(ViewportId::from_hash_of("entry dialog"), vp_builder, move |ctx, _| {
		if ctx.input(|input| input.viewport().close_requested()) {
			let _ = tx.send(None);
			request_repaint();
			return;
		}

		let text_id = Id::new("edit text");
		let mode_id = Id::new("gamemode");
		let mut edit_str = ctx.data_mut(|data| data.get_temp_mut_or_insert_with(text_id, || String::new()).clone());
		let mut mode = ctx.data_mut(|data| *data.get_temp_mut_or(mode_id, Gamemode::GreenOrange));

		CentralPanel::default().show(ctx, |ui| {
			ui.vertical_centered_justified(|ui| {
				ui.label(msg);
				ui.text_edit_singleline(&mut edit_str);
				if ui.button("Confirm").clicked() {
					let _ = tx.send(Some(EntryDialogData {
						textbox: edit_str.clone(),
						mode: mode,
					}));
					request_repaint();
					ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
				}
			});
		});

		if mode_select {
			TopBottomPanel::bottom("mode_select").show(ctx, |ui| {
				ui.vertical_centered_justified(|ui| {
					ComboBox::from_label("Mode")
						.selected_text(format!("{:?}", mode))
						.show_ui(ui, |ui| {
							ui.selectable_value(&mut mode, Gamemode::GreenOrange, "Green Orange");
							ui.selectable_value(&mut mode, Gamemode::WhiteVanilla, "White Vanilla");
							ui.selectable_value(&mut mode, Gamemode::BlackOnion, "Black Onion");
						});
				})
			});
		}

		ctx.data_mut(|data| {
			data.insert_temp(text_id, edit_str);
			data.insert_temp(mode_id, mode);
		});
	});
}
#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Gamemode {
	GreenOrange,
	WhiteVanilla,
	BlackOnion,
}

impl Gamemode {
	fn splits(&self) -> usize {
		match self {
			Gamemode::GreenOrange => 8,
			Gamemode::WhiteVanilla => 26,
			Gamemode::BlackOnion => todo!(),
		}
	}
}
pub struct EntryDialogData {
	pub textbox: String,
	pub mode: Gamemode,
}

fn confirm_dialog(ctx: &Context, tx: Sender<Option<EntryDialogData>>, msg: String) {
	let vp_builder = ViewportBuilder::default()
		.with_title("ZeroSplitter")
		.with_active(true)
		.with_resizable(false)
		.with_minimize_button(false)
		.with_maximize_button(false)
		.with_inner_size([200., 100.]);

	ctx.show_viewport_deferred(ViewportId::from_hash_of("confirm dialog"), vp_builder, move |ctx, _| {
		if ctx.input(|input| input.viewport().close_requested()) {
			let _ = tx.send(None);
			request_repaint();
			return;
		}

		CentralPanel::default().show(ctx, |ui| {
			ui.vertical_centered_justified(|ui| {
				ui.label(msg.clone());
				ui.columns_const(|[left, right]| {
					if left.button("Delete").clicked() {
						let _ = tx.send(Some(EntryDialogData {
							textbox: "Deleted".to_string(),
							mode: Gamemode::GreenOrange,
						}));
						request_repaint();
					} else if right.button("Cancel").clicked() {
						let _ = tx.send(None);
						request_repaint();
					}
				});
			});
		});
	});
}

fn request_repaint() {
	if let Some(ctx) = EGUI_CTX.get() {
		ctx.request_repaint_after(Duration::from_millis(100));
	}
}

/// Translation of function of the same name in ZR.
/// Checkpoints in WV have the same stage/checkpoint as in GO,
/// e.g. the cloudoo segment in stage 1 is technically stage 3 checkpoint 2
/// just like it is in GO. Also, some segments have multiple checkpoints.
/// This function gets the segment number in WV from the GO data.
/// "Realm" seems to indicate when the ship is changed, like in TLB, the dream, or bonus stages
///
/// Someone much smarter than me could try to call this function directly from the
/// game instead of using this.
fn vanilla_get_simstage(stage: u8, checkpoint: u8, checkpoint_sub: u8, realm: u8) -> Result<(u32, u32), ()> {
	Ok(match (stage, checkpoint) {
		//  GO stage/check => WV stage/check
		(1, _) if realm == 3 => (1, 4),
		(1, 0 | 1) => (1, 0),
		(1, 2) => (2, 4),
		(1, 3) => (1, 2),
		(1, 4 | 5 | 6) => (1, 3),

		(2, _) if realm == 3 => (2, 6),
		(2, 0 | 1) => (2, 0),
		(2, 2) => (3, 1),
		(2, 3) => (2, 1),
		(2, 4) => (2, 3),
		(2, 5) => (4, 1),
		(2, 6 | 7 | 8) => (2, 5),

		(3, 0) => (3, 0),
		(3, 1) => (1, 1),
		(3, 2) => (4, 4),
		(3, 3) => (3, 2),
		(3, 4) if checkpoint_sub < 1 => (2, 2),
		(3, 4) => (3, 3),

		(3, 6) => (4, 3),
		(3, 7) => (3, 4),
		(3, 8 | 9) => (3, 5),

		(4, _) if realm == 3 => (3, 6),
		(4, 3) => (3, 6),
		(4, 5 | 6) => (4, 0),
		(4, 7 | 8) => (4, 2),
		(4, 9 | 10) => (4, 5),
		_ => return Err(()),
	})
}

/// Translate sim stage and checkpoint to split count
/// e.g. snake is split 7 (0 indexed)
fn vanilla_get_split_count(simstage: u32, simcheckpoint: u32) -> usize {
	[
		(1, 1),
		(1, 2),
		(1, 3),
		(1, 4),
		(1, 5),
		(2, 1),
		(2, 2),
		(2, 3),
		(2, 4),
		(2, 5),
		(2, 6),
		(2, 7),
		(3, 1),
		(3, 2),
		(3, 3),
		(3, 4),
		(3, 5),
		(3, 6),
		(3, 7),
		(4, 1),
		(4, 2),
		(4, 3),
		(4, 4),
		(4, 5),
		(4, 6),
		(4, 7),
	]
	.iter()
	.position(|&x| x == (simstage, simcheckpoint + 1))
	.unwrap()
}

fn vanilla_stage_from_split(split: usize) -> (u32, u32) {
	[
		(1, 1),
		(1, 2),
		(1, 3),
		(1, 4),
		(1, 5),
		(2, 1),
		(2, 2),
		(2, 3),
		(2, 4),
		(2, 5),
		(2, 6),
		(2, 7),
		(3, 1),
		(3, 2),
		(3, 3),
		(3, 4),
		(3, 5),
		(3, 6),
		(3, 7),
		(4, 1),
		(4, 2),
		(4, 3),
		(4, 4),
		(4, 5),
		(4, 6),
		(4, 7),
	][split]
}

fn vanilla_split_names(split: usize) -> &'static str {
	[
		"1-1", "1-2", "1-3", "1-4", "Bonus 1", "2-1", "2-2", "2-3", "2-4", "2-5", "2-6", "Bonus 2", "3-1", "3-2",
		"3-3", "3-4", "3-5", "3-6", "Bonus 3", "4-1", "4-2", "4-3", "4-4", "4-5", "4-6", "Stage EX",
	][split]
}

fn vanilla_descriptive_split_names(split: usize) -> &'static str {
	[
		"Stage 1 Start",
		"Cloudoos",
		"Arc Adder",
		"Catastrophy",
		"Bonus 1",
		"Stage 2 Start",
		"Box Blockade",
		"Snake",
		"Artypo",
		"Skull Taxis",
		"2nd Apocalypse",
		"Bonus 2",
		"Stage 3 Start",
		"Crab Landing",
		"Plane",
		"Crab",
		"Tank",
		"Grapefruit",
		"Bonus 3",
		"Stage 4 Start",
		"Left Tunnel",
		"Maze",
		"Trains",
		"Knight Ships",
		"Orb Spewer",
		"Stage EX",
	][split]
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy)]
struct OldRun {
	splits: [i32; 8],
	score: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Run {
	splits: Vec<i32>,
	score: i32,
	mode: Gamemode
}

impl Run {
	fn new(mode: Gamemode) -> Self {
		Run {
			splits: vec![0; mode.splits()],
			score: 0,
			mode
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OldCategory {
	personal_best: OldRun,
	best_splits: [i32; 8],
	name: String,
}

impl OldRun {
	fn to_new(self) -> Run {
		Run {
			splits: self.splits.to_vec(),
			score: self.score,
			mode: Gamemode::GreenOrange
		}
	}
}

impl OldCategory {
	fn to_new(&self, mode: Gamemode) -> Category {
		Category {
			personal_best: self.personal_best.to_new(),
			best_splits: self.best_splits.to_vec(),
			name: self.name.clone(),
			mode,
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Category {
	personal_best: Run,
	best_splits: Vec<i32>,
	name: String,
	mode: Gamemode,
}

impl Category {
	fn new(name: String, mode: Gamemode) -> Self {
		Category {
			personal_best: Run::new(mode),
			best_splits: vec![0; mode.splits()],
			name,
			mode,
		}
	}

	fn update_from_run(&mut self, run: &Run) {
		if run.mode != self.mode {
			return
		}

		if run.score > self.personal_best.score {
			self.personal_best = run.clone();
		}

		for (best, new) in self.best_splits.iter_mut().zip(run.splits.iter()) {
			if *new > *best {
				*best = *new;
			}
		}
	}
}
