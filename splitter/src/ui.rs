use std::{sync::mpsc::Sender, time::Duration};

use eframe::egui::{CentralPanel, ComboBox, Context, Id, TopBottomPanel, ViewportBuilder, ViewportId};

use crate::{EGUI_CTX, EntryDialogData, Gamemode};

/// Category entry dialoge menu with a gamemode dropdown.
pub fn category_maker_dialog(ctx: &Context, tx: Sender<Option<EntryDialogData>>, msg: &'static str, mode_select: bool) {
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
		let mut edit_str = ctx.data_mut(|data| data.get_temp_mut_or_insert_with(text_id, String::new).clone());
		let mut mode = ctx.data_mut(|data| *data.get_temp_mut_or(mode_id, Gamemode::GreenOrange));

		CentralPanel::default().show(ctx, |ui| {
			ui.vertical_centered_justified(|ui| {
				ui.label(msg);
				ui.text_edit_singleline(&mut edit_str);
				if ui.button("Confirm").clicked() {
					let _ = tx.send(Some(EntryDialogData {
						textbox: edit_str.clone(),
						mode,
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
						.selected_text(format!("{mode:?}"))
						.show_ui(ui, |ui| {
							ui.selectable_value(&mut mode, Gamemode::GreenOrange, "Green Orange");
							ui.selectable_value(&mut mode, Gamemode::WhiteVanilla, "White Vanilla");
							// ui.selectable_value(&mut mode, Gamemode::BlackOnion, "Black Onion");
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

pub fn confirm_dialog(ctx: &Context, tx: Sender<Option<EntryDialogData>>, msg: String) {
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
