use std::{
	env,
	fs::{self, File},
	net::UdpSocket,
	sync::{
		LazyLock, OnceLock,
		mpsc::{self, Receiver, Sender},
	},
	thread,
	time::Duration,
};

use common::FrameData;
use eframe::{
	NativeOptions,
	egui::{Context, IconData, ThemePreference, ViewportBuilder},
};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use toml::de::Error;

use crate::{config::CONFIG, database::Database, run::Run, theme::zeroranger_visuals};

mod app;
mod config;
mod database;
mod hook;
mod run;
mod system;
mod theme;
mod ui;

const SPLIT_DELAY_FRAMES: u32 = 20;

static EGUI_CTX: OnceLock<Context> = OnceLock::new();

fn main() {
	config::load_config().unwrap();
	#[cfg(debug_assertions)]
	unsafe {
		env::set_var("RUST_BACKTRACE", "1");
	}

	pretty_env_logger::init();

	let options = NativeOptions {
		viewport: ViewportBuilder::default()
			.with_inner_size([300., 300.])
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
			c.egui_ctx.set_visuals(zeroranger_visuals());
			c.egui_ctx.set_zoom_factor(CONFIG.get().unwrap().zoom_level);
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
	categories: CategoryManager,
	data_source: Receiver<FrameData>,
	last_frame: FrameData,
	run: Run,
	waiting_for_category: bool,
	waiting_for_rename: bool,
	waiting_for_confirm: bool,
	dialog_rx: Receiver<Option<EntryDialogData>>,
	dialog_tx: Sender<Option<EntryDialogData>>,
	relative_score: bool,
	show_gold_split: bool,
	split_delay: Option<u32>,
	names: bool,
	db: Database,
}

impl ZeroSplitter {
	fn new(data_source: Receiver<FrameData>, db: Database) -> Self {
		let (tx, rx) = mpsc::channel();
		let mut zerosplitter = Self {
			categories: CategoryManager::init(),
			data_source,
			last_frame: FrameData::default(),
			run: Run::Inactive,
			dialog_rx: rx,
			dialog_tx: tx,
			waiting_for_category: false,
			waiting_for_rename: false,
			waiting_for_confirm: false,
			relative_score: true,
			show_gold_split: true,
			split_delay: None,
			names: false,
			db,
		};

		zerosplitter.categories.load(&zerosplitter.db).unwrap();

		zerosplitter
	}

	fn load(data_source: Receiver<FrameData>) -> Self {
		let db = Database::init().unwrap();

		Self::new(data_source, db.clone())
	}

	fn save_splits(&mut self) {
		if self.run.is_active() && self.run.splits().unwrap().iter().sum::<i32>() > 0 {
			debug!("Saving splits");

			if let Err(err) = self.db.insert_run(&self.categories, &self.run) {
				error!("Error writing run to database: {err}");
			}
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
		// Skip update if current category isn't Green Orange or if on menu
		if self.categories.current().mode != Gamemode::GreenOrange || frame.is_menu() {
			return;
		}

		// Reset if we just left the menu or returned to 1-1
		if frame.stage != self.last_frame.stage && (self.last_frame.is_menu() || frame.is_first_stage()) {
			self.reset();
			self.run.start(frame);
			self.categories.refresh_comparison(&self.db).unwrap();
		}

		if !frame.is_menu() && self.run.is_active() {
			let frame_split = (frame.stage - 1 - frame.game_loop) as usize;

			if frame_split >= 8 {
				// TLB or credits
				return;
			}

			// Split if necessary
			if (frame.stage != self.last_frame.stage) && !self.last_frame.is_menu() {
				self.run.split().unwrap();
			}

			// Update run and split scores
			self.run.update(frame).unwrap();
		} else {
			// End the run if we're back on the menu
			self.end_run();
		}
	}

	fn update_whitevanilla(&mut self, frame: FrameData) {
		// Skip update if current category isn't White Vanilla or if on menu
		if self.categories.current().mode != Gamemode::WhiteVanilla || frame.is_menu() {
			return;
		}

		// Reset if we returned to 1-1
		if frame.total_score() == 0 && self.last_frame.total_score() > 0 || self.last_frame.is_menu() {
			self.reset();
			self.run.start(frame);
			self.run
				.set_split(match frame.stage {
					1 => 0,
					2 => 5,
					3 => 12,
					4 => 19,
					_ => panic!("Stage out of bounds! {}", frame.stage),
				})
				.unwrap();
			self.categories.refresh_comparison(&self.db).unwrap();
			return;
		}

		if !frame.is_menu() && !(self.run == Run::Inactive) {
			// Split if necessary; score requirement prevents spurious splits after a reset
			if frame.timer_wave == 0 && self.last_frame.timer_wave != 0 && frame.total_score() > 0 {
				self.split_delay = Some(SPLIT_DELAY_FRAMES);
			}

			if let Some(split_delay) = self.split_delay {
				if split_delay > 1 {
					self.split_delay = Some(split_delay - 1)
				} else {
					self.run.split().unwrap();
					self.split_delay = None
				}
			}

			// Update run and split scores
			self.run.update(frame).unwrap();
		} else if frame.is_menu() {
			// End the run if we're back on the menu
			self.end_run();
		}
	}

	fn reset(&mut self) {
		self.save_splits();
		self.run.reset();
	}

	fn end_run(&mut self) {
		self.save_splits();
		self.run.stop()
	}
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

impl From<i8> for Gamemode {
	fn from(value: i8) -> Self {
		match value {
			-1 => Self::WhiteVanilla,
			0 => Self::GreenOrange,
			1 => panic!("illegal black onion detected"),
			_ => panic!(),
		}
	}
}
pub struct EntryDialogData {
	pub textbox: String,
	pub mode: Gamemode,
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

// #[derive(Debug, Serialize, Deserialize, Clone)]
// struct Run {
// 	splits: Vec<i32>,
// 	score: i32,
// 	mode: Gamemode,
// 	active: bool
// }

// impl Run {
// 	fn new(mode: Gamemode) -> Self {
// 		Run {
// 			splits: vec![0; mode.splits()],
// 			score: 0,
// 			mode,
// 			active: false,
// 		}
// 	}

// 	fn start(&mut self, category: &CategoryManager) {
// 		if category.current().mode == self.mode {
// 			self.splits = vec![0; self.mode.splits()];
// 			self.score = 0;
// 			self.active = true
// 		}
// 	}

// 	fn stop(&mut self) {
// 		self.active = false
// 	}
// }

struct CategoryManager {
	categories: Vec<Category>,
	current: usize,
	comparison_cache: Vec<i32>,
}

impl CategoryManager {
	fn init() -> Self {
		CategoryManager {
			categories: Vec::new(),
			current: 0,
			comparison_cache: Vec::new(),
		}
	}

	fn current(&self) -> &Category {
		&self.categories.get(self.current).unwrap()
	}

	fn current_mut(&mut self) -> &mut Category {
		&mut self.categories[self.current]
	}

	#[must_use]
	pub fn push(&mut self, name: String, mode: Gamemode, db: &Database) -> Result<(), ZeroError> {
		let id = db
			.insert_new_category(name.clone(), mode)
			.map_err(ZeroError::DatabaseError)?;
		self.categories.push(Category { name, mode, id });
		Ok(())
	}

	pub fn index(&self, index: usize) -> Option<&Category> {
		self.categories.get(index)
	}

	pub fn len(&self) -> usize {
		self.categories.len()
	}

	/// Populate the CategoryManager with data from the database
	pub fn load(&mut self, db: &Database) -> Result<(), ZeroError> {
		self.categories = db.get_categories().map_err(ZeroError::DatabaseError)?;
		Ok(())
	}

	pub fn delete_current(&mut self, db: &Database) -> Result<usize, ZeroError> {
		if self.categories.len() > 1 {
			db.delete_category(self.categories.remove(self.current))
				.map_err(ZeroError::DatabaseError)
		} else {
			Err(ZeroError::Illegal)
		}
	}

	pub fn rename_current(&mut self, db: &Database, new_name: String) -> Result<usize, ZeroError> {
		self.current_mut().name = new_name.clone();
		db.rename_category(self.current(), new_name)
			.map_err(ZeroError::DatabaseError)
	}

	pub fn set_current(&mut self, new_idx: usize, db: &Database) -> Result<(), ZeroError> {
		if new_idx >= self.categories.len() {
			return Err(ZeroError::CategoryOutOfRange);
		}
		self.current = new_idx;
		self.refresh_comparison(db)?;

		Ok(())
	}

	pub fn get_comparison(&self) -> &Vec<i32> {
		if self.comparison_cache.len() == 0 {
			panic!()
		}
		&self.comparison_cache
	}

	pub fn refresh_comparison(&mut self, db: &Database) -> Result<(), ZeroError> {
		self.comparison_cache = match db.get_pb_run(self) {
			Ok((scores, total, mode)) if mode == self.current().mode => scores,
			Ok(_) => return Err(ZeroError::DifficultyMismatch),
			Err(rusqlite::Error::QueryReturnedNoRows) => vec![0; self.current().mode.splits()],
			Err(e) => return Err(ZeroError::DatabaseError(e)),
		};
		Ok(())
	}
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Category {
	name: String,
	mode: Gamemode,
	id: i64,
}

#[derive(Debug)]
#[non_exhaustive]
enum ZeroError {
	Illegal,
	DatabaseError(rusqlite::Error),
	RunInactive,
	DifficultyMismatch,
	SplitOutOfRange,
	CategoryOutOfRange,
	IOError(std::io::Error),
	TOMLError(toml::de::Error),
	StaticAlreadyInit,
}
