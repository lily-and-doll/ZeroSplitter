use eframe::egui::{
	self, Color32, CornerRadius, Stroke,
	style::{self, WidgetVisuals},
};

#[allow(unused)]
pub const DARK_GREEN: Color32 = Color32::from_rgb(0, 0x4f, 0x4d);
#[allow(unused)]
pub const GREEN: Color32 = Color32::from_rgb(0, 0x94, 0x79);
#[allow(unused)]
pub const LIGHT_ORANGE: Color32 = Color32::from_rgb(0xff, 0xc0, 0x73);
#[allow(unused)]
pub const DARK_ORANGE: Color32 = Color32::from_rgb(0xff, 0x80, 0);
#[allow(unused)]
pub const DARKER_ORANGE: Color32 = Color32::from_rgb(0xdd, 0x59, 0x28);
#[allow(unused)]
pub const ORANGEST: Color32 = Color32::from_rgb(0xad, 0x2f, 0x17);
#[allow(unused)]
pub const DARKER_GREEN: Color32 = Color32::from_rgb(0x00, 0x32, 0x32);
#[allow(unused)]
pub const GREENEST: Color32 = Color32::from_rgb(0x00, 0x1d, 0x23);
#[allow(unused)]
pub const WHITE: Color32 = Color32::WHITE;
#[allow(unused)]
pub const BLACK: Color32 = Color32::BLACK;

pub fn zeroranger_visuals() -> egui::Visuals {
	egui::Visuals {
		window_fill: BLACK,
		extreme_bg_color: GREENEST,
		panel_fill: BLACK,
		widgets: egui::style::Widgets {
			inactive: egui::style::WidgetVisuals {
				bg_fill: GREENEST,
				weak_bg_fill: GREENEST,
				bg_stroke: Stroke { ..Default::default() },
				corner_radius: CornerRadius::ZERO,
				fg_stroke: Stroke {
					width: 1.5,
					color: DARK_ORANGE,
				},
				expansion: 0.0,
			},
			active: egui::style::WidgetVisuals {
				bg_fill: GREENEST,
				weak_bg_fill: GREENEST,
				bg_stroke: Stroke { ..Default::default() },
				corner_radius: CornerRadius::ZERO,
				fg_stroke: Stroke {
					width: 1.0,
					color: DARK_ORANGE,
				},
				expansion: 0.0,
			},
			hovered: WidgetVisuals {
				bg_fill: LIGHT_ORANGE,
				weak_bg_fill: LIGHT_ORANGE,
				bg_stroke: Stroke { ..Default::default() },
				corner_radius: CornerRadius::ZERO,
				fg_stroke: Stroke {
					width: 1.0,
					color: DARK_GREEN,
				},
				expansion: 0.0,
			},
			noninteractive: WidgetVisuals {
				bg_fill: LIGHT_ORANGE,
				weak_bg_fill: LIGHT_ORANGE,
				bg_stroke: Stroke {
					color: GREEN,
					width: 1.0,
					..Default::default()
				},
				corner_radius: CornerRadius::ZERO,
				fg_stroke: Stroke {
					width: 1.0,
					color: DARKER_ORANGE,
				},
				expansion: 0.0,
			},
			open: WidgetVisuals {
				bg_fill: LIGHT_ORANGE,
				weak_bg_fill: LIGHT_ORANGE,
				bg_stroke: Stroke { ..Default::default() },
				corner_radius: CornerRadius::ZERO,
				fg_stroke: Stroke {
					width: 1.0,
					color: DARKER_ORANGE,
				},
				expansion: 0.0,
			},
		},
		selection: style::Selection {
			bg_fill: DARK_ORANGE,
			stroke: Stroke {
				width: 1.0,
				color: GREENEST,
			},
		},
		..Default::default()
	}
}
