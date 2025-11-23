use eframe::{
	App, Frame,
	egui::{Align, CentralPanel, Color32, ComboBox, Context, Id, Layout, Sides, Ui},
};

use crate::{
	Gamemode, Run, ZeroError, ZeroSplitter,
	config::CONFIG,
	theme::{DARK_GREEN, DARK_ORANGE, DARKER_GREEN, DARKER_ORANGE, GREEN, LIGHT_ORANGE},
	ui::{category_maker_dialog, confirm_dialog},
	vanilla_descriptive_split_names, vanilla_split_names,
};

impl App for ZeroSplitter {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		while let Ok(data) = self.data_source.try_recv() {
			self.update_frame(data);
		}

		// Detect gamemode change persist between frames
		let prev_mode_id = Id::new("prev_mode");
		let cur_mode = self.categories.current().mode;
		if let Some(prev_mode) = ctx.data(|data| data.get_temp::<Gamemode>(prev_mode_id))
			&& prev_mode != cur_mode
		{
			let zoom_level = CONFIG.get().unwrap().zoom_level;
			let min_size = match self.categories.current().mode {
				Gamemode::GreenOrange => eframe::egui::Vec2 {
					x: 300.0 * zoom_level,
					y: 300.0 * zoom_level,
				},
				Gamemode::WhiteVanilla => eframe::egui::Vec2 {
					x: 300.0 * zoom_level,
					y: 650.0 * zoom_level,
				},
				Gamemode::BlackOnion => todo!(),
			};
			ctx.send_viewport_cmd(eframe::egui::ViewportCommand::MinInnerSize(min_size));
			ctx.send_viewport_cmd(eframe::egui::ViewportCommand::InnerSize(min_size));
			self.reset();
		}
		ctx.data_mut(|data| data.insert_temp(prev_mode_id, cur_mode));

		CentralPanel::default().show(ctx, |ui| {
			ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
				ui.horizontal(|ui| {
					ui.toggle_value(&mut self.relative_score, "RELATIVE")
						.on_hover_text("Display relative score per split or running total of score");
					ui.toggle_value(&mut self.show_gold_split, "BEST SPLITS")
						.on_hover_text("Show your PB's splits or your best splits on the left");
					ui.toggle_value(&mut self.names, "NAMES")
						.on_hover_text("Toggle descriptive or number names for WV splits");
				});
				ui.horizontal(|ui| {
					ui.label("Category: ");
					{
						let len = self.categories.len();
						let mut cat_idx = self.categories.current;
						ComboBox::from_label("")
							.show_index(ui, &mut cat_idx, len, |i| &self.categories.index(i).unwrap().name);
						if self.categories.current != cat_idx {
							self.end_run();
							self.categories.set_current(cat_idx, &self.db).unwrap();
						}
					}

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

				if let Ok(data) = self.calculate_splits() {
					self.display_splits(ui, data);
				};

				ui.label(format!(
					"Personal Best: {}",
					self.db.get_pb_run(&self.categories).map_or(0, |r| r.1)
				));
				ui.label(format!(
					"Sum of Best: {}",
					self.db.get_gold_splits(&self.categories).map_or(0, |s| s.iter().sum())
				));
			});
		});

		if self.waiting_for_category {
			if let Ok(new_category) = self.dialog_rx.try_recv() {
				if let Some(data) = new_category {
					self.categories.push(data.textbox, data.mode, &self.db).unwrap();
				}
				self.waiting_for_category = false;
			} else {
				category_maker_dialog(ctx, self.dialog_tx.clone(), "Enter new category name", true);
			}
		}

		if self.waiting_for_rename {
			if let Ok(rename_category) = self.dialog_rx.try_recv() {
				if let Some(data) = rename_category {
					self.categories.rename_current(&self.db, data.textbox).unwrap();
				}
				self.waiting_for_rename = false;
			} else {
				category_maker_dialog(ctx, self.dialog_tx.clone(), "Enter new name for category", false);
			}
		}

		if self.waiting_for_confirm {
			if let Ok(Some(confirmation)) = self.dialog_rx.try_recv() {
				if confirmation.textbox == "Deleted" {
					self.categories.delete_current(&self.db).unwrap();
				}
				self.waiting_for_confirm = false;
			} else {
				confirm_dialog(
					ctx,
					self.dialog_tx.clone(),
					format!(
						"Are you sure you want to delete category {}?",
						self.categories.current().name
					),
				);
			}
		}
	}

	fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
		if let Run::Active { .. } = self.run {
			self.save_splits();
		}
	}
}

impl ZeroSplitter {
	fn calculate_splits(&self) -> Result<Vec<(i32, i32, i32)>, ZeroError> {
		// relative split: score gained during one split
		// absolute split: total score during one split
		let raw_splits = self.run.splits()?;
		let mut ret = Vec::new();
		for (i, rel_split, abs_split) in raw_splits.iter().enumerate().map(|(i, &s)| {
			(i, s, {
				raw_splits
					.clone()
					.iter()
					.enumerate()
					.take_while(|&(idx, _)| idx <= i)
					.fold(0, |acc, (_, &s)| acc + s)
			})
		}) {
			let split = if self.relative_score { rel_split } else { abs_split };
			// Get relative/absolute gold split
			// Gold split = high score of this split in any run
			let best_splits = self.db.get_gold_splits(&self.categories);
			let gold_split = match (best_splits, self.relative_score) {
				(Ok(splits), true) => splits[i],
				(Ok(splits), false) => splits
					.iter()
					.enumerate()
					.take_while(|&(idx, _)| idx <= i)
					.fold(0, |acc, (_, &s)| acc + s),
				_ => 0,
			};
			// Get relative/absolute split in the PB
			// PB split = score of this split in the PB run
			let compare = self.categories.get_comparison();
			let rel_pb_split = compare[i];
			let abs_pb_split = compare
				.iter()
				.enumerate()
				.take_while(|&(idx, _)| idx <= i)
				.fold(0, |acc, (_, &s)| acc + s);

			let pb_split = if self.relative_score {
				rel_pb_split
			} else {
				abs_pb_split
			};

			ret.push((gold_split, split, pb_split));
		}
		Ok(ret)
	}

	fn display_splits(&self, ui: &mut Ui, split_data: Vec<(i32, i32, i32)>) {
		let current_split = self.run.current_split().unwrap_or(0);

		for (n, &(gold_score, current_score, compare_score)) in split_data.iter().enumerate() {
			// translate split number to stage/loop for GO
			let stage_n = (n & 3) + 1;
			let loop_n = (n >> 2) + 1;
			Sides::new().show(
				ui,
				|left| {
					match self.categories.current().mode {
						Gamemode::GreenOrange => left.label(format!("{loop_n}-{stage_n}")),
						Gamemode::WhiteVanilla => {
							if self.names {
								left.label(vanilla_descriptive_split_names(n))
							} else {
								left.label(vanilla_split_names(n))
							}
						}
						Gamemode::BlackOnion => todo!(),
					};

					if self.show_gold_split {
						if gold_score > 0 {
							left.colored_label(GREEN, gold_score.to_string());
						}
					} else if compare_score > 0 {
						left.colored_label(GREEN, compare_score.to_string());
					}
				},
				|right| {
					// Only write splits up to the current split
					if n <= self.run.current_split().unwrap() {
						// Set color of split (rightmost number)
						let split_color = if current_split == n && self.split_delay.is_none() {
							Color32::WHITE
						} else if current_score >= gold_score {
							DARKER_ORANGE
						} else {
							DARK_ORANGE
						};

						right.colored_label(split_color, current_score.to_string());

						if n < current_split {
							// past split, we should show a diff
							let diff = current_score - compare_score;
							if self.relative_score {
								let diff_color = if diff > 0 {
									LIGHT_ORANGE
								} else if diff == 0 {
									Color32::WHITE
								} else {
									DARK_GREEN
								};
								right.colored_label(diff_color, format!("{diff:+}"));
							} else {
								let &(_, prev_score, prev_compare) =
									n.checked_sub(1).map_or(&(0, 0, 0), |n| split_data.get(n).unwrap());
								let rel_diff = (current_score - prev_score) - (compare_score - prev_compare);
								let diff_color = if diff > 0 {
									if rel_diff > 0 { LIGHT_ORANGE } else { DARKER_ORANGE }
								} else if diff == 0 {
									Color32::WHITE
								} else if rel_diff > 0 {
									DARK_GREEN
								} else {
									DARKER_GREEN
								};
								right.colored_label(diff_color, format!("{diff:+}"));
							}
						}
					} else {
						right.colored_label(DARK_GREEN, "--");
					}
				},
			);
		}
	}
}
