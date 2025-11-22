use common::FrameData;

use crate::{Gamemode, ZeroError};

#[derive(Debug, PartialEq)]
pub enum Run {
	Inactive,
	Active {
		difficulty: Gamemode,
		splits: Vec<i32>,
		mults: Vec<u32>,
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

	pub fn splits(&self) -> Result<Vec<i32>, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { splits, .. } => Ok(splits.to_vec()),
		}
	}

	pub fn mults(&self) -> Result<Vec<u32>, ZeroError> {
		match self {
			Run::Inactive => Err(ZeroError::RunInactive),
			Run::Active { mults, .. } => Ok(mults.to_vec()),
		}
	}

	pub fn start(&mut self, frame: FrameData) {
		let mode = frame.difficulty.into();
		*self = Self::Active {
			difficulty: mode,
			splits: vec![0; mode.splits()],
			mults: vec![0; mode.splits()],
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
				splits: vec![0; difficulty.splits()],
				mults: vec![0; difficulty.splits()],
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
			mults,
		} = self
		{
			if *difficulty == Gamemode::from(frame.difficulty) {
				*score = frame.total_score();
				if *split_base_score > *score {
					*split_base_score = 0
				}
				splits[*current_split] = frame.total_score() - *split_base_score;
				mults[*current_split] = frame.multiplier_one;
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
