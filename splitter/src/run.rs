use common::FrameData;

use crate::{Gamemode, ZeroError};

#[derive(Debug, PartialEq)]
pub enum Run {
	Inactive,
	Active {
		difficulty: Gamemode,
		splits: Vec<SplitData>,
		score: i32,
		current_split: usize,
		split_base_score: i32,
	},
}

impl Run {
	pub fn score(&self) -> Option<i32> {
		match *self {
			Run::Inactive => None,
			Run::Active { score, .. } => Some(score),
		}
	}

	pub fn scores(&self) -> Result<Vec<i32>, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { splits, .. } => Ok(splits.iter().map(|x| x.score).collect()),
		}
	}

	pub fn splits(&self) -> Result<Vec<SplitData>, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { splits, .. } => Ok(splits.iter().map(|&s| s).collect()),
		}
	}

	pub fn mults(&self) -> Result<Vec<u32>, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { splits, .. } => Ok(splits.iter().map(|s| s.mult).collect()),
		}
	}

	pub fn start(&mut self, frame: FrameData) {
		let mode = frame.difficulty.into();
		*self = Self::Active {
			difficulty: mode,
			splits: vec![Default::default(); mode.splits()],
			score: 0,
			current_split: 0,
			split_base_score: 0,
		};
	}

	pub fn stop(&mut self) {
		*self = Self::Inactive
	}

	pub fn reset(&mut self) {
		*self = match *self {
			Run::Inactive => Run::Inactive,
			Run::Active { difficulty, .. } => Run::Active {
				difficulty: difficulty,
				splits: vec![Default::default(); difficulty.splits()],
				score: 0,
				current_split: 0,
				split_base_score: 0,
			},
		}
	}

	pub fn update(&mut self, frame: FrameData) -> Result<(), ZeroError> {
		if let Self::Active {
			difficulty,
			splits,
			score,
			current_split,
			split_base_score,
		} = self
		{
			if *difficulty == Gamemode::from(frame.difficulty) {
				*score = frame.total_score();
				if *split_base_score > *score {
					*split_base_score = 0
				}
				let data = SplitData::new(
					frame.total_score() - *split_base_score,
					frame.multiplier_one,
					frame.pattern_rank,
					frame.dynamic_rank,
				);
				*splits.get_mut(*current_split).unwrap() = data;

				Ok(())
			} else {
				Err(ZeroError::DifficultyMismatch)
			}
		} else {
			Err(ZeroError::RunInactive)
		}
	}

	pub fn split(&mut self) -> Result<(), ZeroError> {
		if let Self::Active {
			current_split,
			difficulty,
			score,
			split_base_score,
			..
		} = self
		{
			if *current_split < difficulty.splits() {
				*current_split += 1;
				*split_base_score = *score;
				Ok(())
			} else {
				Err(ZeroError::SplitOutOfRange)
			}
		} else {
			Err(ZeroError::RunInactive)
		}
	}

	pub fn set_split(&mut self, new_split: usize) -> Result<(), ZeroError> {
		if let Self::Active {
			current_split,
			difficulty,
			..
		} = self
		{
			if new_split < difficulty.splits() {
				*current_split = new_split;
				Ok(())
			} else {
				Err(ZeroError::SplitOutOfRange)
			}
		} else {
			Err(ZeroError::RunInactive)
		}
	}
	pub fn current_split(&self) -> Result<usize, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { current_split, .. } => Ok(*current_split),
		}
	}

	pub fn is_active(&self) -> bool {
		match self {
			Run::Inactive => false,
			Run::Active { .. } => true,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitData {
	pub score: i32,
	pub mult: u32,
	pub pattern_rank: f32,
	pub dynamic_rank: f32,
}

impl SplitData {
	fn new(score: i32, mult: u32, pattern_rank: f32, dynamic_rank: f32) -> Self {
		Self {
			score,
			mult,
			pattern_rank,
			dynamic_rank,
		}
	}
}

impl Default for SplitData {
	fn default() -> Self {
		Self {
			score: Default::default(),
			mult: Default::default(),
			pattern_rank: Default::default(),
			dynamic_rank: Default::default(),
		}
	}
}
