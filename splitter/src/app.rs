use eframe::{App, Frame, egui::{Align, CentralPanel, Color32, ComboBox, Context, Id, Layout, Sides}};

use crate::{Category, DARK_GREEN, DARK_ORANGE, DARKER_ORANGE, GREEN, GREENEST, Gamemode, LIGHT_ORANGE, ZeroSplitter, ui::{category_maker_dialog, confirm_dialog}, vanilla_split_names};

impl App for ZeroSplitter {
	fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
		while let Ok(data) = self.data_source.try_recv() {
			self.update_frame(data);
		}

		// Detect gamemode change persist between frames
		let prev_mode_id = Id::new("prev_mode");
		let cur_mode = self.categories[self.current_category].mode;
		if let Some(prev_mode) = ctx.data(|data| data.get_temp::<Gamemode>(prev_mode_id)) {
			if prev_mode != cur_mode {
				let min_size = match self.categories[self.current_category].mode {
					Gamemode::GreenOrange => eframe::egui::Vec2 { x: 300.0, y: 300.0 },
					Gamemode::WhiteVanilla => eframe::egui::Vec2 { x: 300.0, y: 650.0 },
					Gamemode::BlackOnion => todo!(),
				};
				ctx.send_viewport_cmd(eframe::egui::ViewportCommand::MinInnerSize(min_size));
				ctx.send_viewport_cmd(eframe::egui::ViewportCommand::InnerSize(min_size));
				self.reset();
			}
		}
		ctx.data_mut(|data| data.insert_temp(prev_mode_id, cur_mode));

		let cur_category = &self.categories[self.current_category];

		CentralPanel::default().show(ctx, |ui| {
			ui.visuals_mut().selection.bg_fill = DARK_ORANGE;
			ui.visuals_mut().selection.stroke.color = GREENEST;
			ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
				ui.horizontal(|ui| {
					ui.toggle_value(&mut self.relative_score, "RELATIVE")
						.on_hover_text("Display relative score per split or running total of score");
					ui.toggle_value(&mut self.show_gold_split, "BEST SPLITS")
						.on_hover_text("Show your PB's splits or your best splits on the left");
				});
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

				for (i, split) in self.current_run.splits.iter().enumerate().map(|(i, &s)| {
					// Switch current split score between modes
					if self.relative_score {
						(i, s)
					} else {
						// Get total score up to the current split
						(
							i,
							self.current_run
								.splits
								.iter()
								.enumerate()
								.take_while(|&(idx, _)| idx <= i)
								.fold(0, |acc, (_, &s)| acc + s),
						)
					}
				}) {
					// translate split number to stage/loop for GO
					let stage_n = (i & 3) + 1;
					let loop_n = (i >> 2) + 1;
					// Get relative/absolute gold split
					// Gold split = high score of this split in any run
					let gold_split = if self.relative_score {
						cur_category.best_splits[i]
					} else {
						cur_category
							.best_splits
							.iter()
							.enumerate()
							.take_while(|&(idx, _)| idx <= i)
							.fold(0, |acc, (_, &s)| acc + s)
					};
					// Get relative/absolute split in the PB
					// PB split = score of this split in the PB run
					let pb_split = if self.relative_score {
						self.comparison.personal_best.splits[i]
					} else {
						self.comparison
							.personal_best
							.splits
							.iter()
							.enumerate()
							.take_while(|&(idx, _)| idx <= i)
							.fold(0, |acc, (_, &s)| acc + s)
					};

					Sides::new().show(
						ui,
						|left| {
							match cur_category.mode {
								Gamemode::GreenOrange => left.label(format!("{}-{}", loop_n, stage_n)),
								Gamemode::WhiteVanilla => left.label(vanilla_split_names(i)),
								Gamemode::BlackOnion => todo!(),
							};

							if self.show_gold_split {
								if gold_split > 0 {
									left.colored_label(GREEN, gold_split.to_string());
								}
							} else {
								if pb_split > 0 {
									left.colored_label(GREEN, pb_split.to_string());
								}
							}
						},
						|right| {
							// Only write splits up to the current split
							if i <= self.current_split.unwrap_or(0) {
								// Set color of split (rightmost number)
								let split_color = if self.current_split == Some(i) && self.split_delay.is_none() {
									Color32::WHITE
								} else if split >= gold_split {
									DARKER_ORANGE
								} else {
									DARK_ORANGE
								};

								right.colored_label(split_color, split.to_string());

								if i < self.current_split.unwrap_or(0) {
									// past split, we should show a diff
									let diff = split - pb_split;
									let diff_color = if diff > 0 {
										LIGHT_ORANGE
									} else if diff == 0 {
										Color32::WHITE
									} else {
										DARK_GREEN
									};
									right.colored_label(diff_color, format!("{diff:+}"));
								}
							} else {
								right.colored_label(DARK_GREEN, "--");
							}
						},
					);
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