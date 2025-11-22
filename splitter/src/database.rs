use std::{fmt::Error, sync::Arc};

use log::error;
use rusqlite::{
	Connection, Result, ToSql, params,
	types::{FromSql, ValueRef},
};

use crate::{Category, CategoryManager, Gamemode, Run};

#[derive(Clone)]
pub struct Database {
	conn: Arc<Connection>,
}

impl Database {
	pub fn init() -> Result<Self> {
		let database = Database {
			conn: Arc::new(Connection::open("./sqlite.db3")?),
		};

		let schema_version = database
			.conn
			.query_one("PRAGMA user_version", (), |row| row.get::<_, i32>(0))
			.unwrap();
		// #[cfg(debug_assertions)]
		// {
		// 	database.conn.execute("DROP TABLE IF EXISTS splits", ()).unwrap();
		// 	database.conn.execute("DROP TABLE IF EXISTS runs", ()).unwrap();
		// 	database.conn.execute("DROP TABLE IF EXISTS categories", ()).unwrap();
		// }
		if !database.conn.table_exists(Some("main"), "categories")? {
			if let Err(err) = database.create_tables() {
				error!("Error creating tables: {}", err)
			};
			database.insert_new_category("default".to_owned(), Gamemode::GreenOrange)?;
		}

		Ok(database)
	}
	#[must_use]
	pub fn create_tables(&self) -> Result<()> {
		self.conn.execute(
			"
    CREATE TABLE IF NOT EXISTS categories (
        id          INTEGER PRIMARY KEY,
        name        TEXT UNIQUE NOT NULL,
        mode        INTEGER NOT NULL
    )
    ",
			(),
		)?;

		self.conn.execute(
			"
    CREATE TABLE IF NOT EXISTS runs (
        id          INTEGER PRIMARY KEY,
        category    INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE
    ) 
    ",
			(),
		)?;

		self.conn.execute(
			"
    CREATE TABLE IF NOT EXISTS splits (
        id          INTEGER PRIMARY KEY,
        split_num   INTEGER NOT NULL,
        score       INTEGER NOT NULL,
        hits        INTEGER,
        mult        REAL,
        run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE
    )
    ",
			(),
		)?;

		Ok(())
	}

	#[must_use]
	pub fn insert_current_category(&self, category: &CategoryManager) -> Result<usize> {
		let category = category.current();
		self.conn.execute(
			"INSERT OR IGNORE INTO categories VALUES(NULL, ?1, ?2)",
			params![category.name, category.mode],
		)
	}
	#[must_use]
	pub fn insert_new_category(&self, name: String, mode: Gamemode) -> Result<i64> {
		self.conn
			.execute("INSERT INTO categories VALUES(NULL, ?1, ?2)", params![name, mode])?;

		Ok(self.conn.last_insert_rowid())
	}
	#[must_use]
	pub fn delete_category(&self, category: Category) -> Result<usize> {
		self.conn
			.execute("DELETE FROM categories WHERE id = ?1", params![category.id])
	}
	#[must_use]
	pub fn rename_category(&self, category: &Category, new_name: String) -> Result<usize> {
		self.conn.execute(
			"UPDATE categories SET name='?1' WHERE id=?2",
			params![new_name, category.id],
		)
	}
	#[must_use]
	pub fn get_categories(&self) -> Result<Vec<Category>> {
		let mut statement = self.conn.prepare("SELECT name, mode, id FROM categories")?;
		let rows = statement.query_map((), |row| {
			Ok((
				row.get::<_, String>(0)?,
				row.get::<_, Gamemode>(1)?,
				row.get::<_, i64>(2)?,
			))
		})?;
		let categories = rows
			.map(|r| r.unwrap())
			.map(|(name, mode, id)| Category { id, mode, name })
			.collect::<Vec<Category>>();
		Ok(categories)
	}
	#[must_use]
	pub fn insert_run(&self, category: &CategoryManager, run: &Run) -> Result<()> {
		let category = category.current();
		self.conn.execute("BEGIN TRANSACTION", ())?;

		match (|| {
			let mut stmt = self.conn.prepare("SELECT id FROM categories WHERE name = ?1")?;
			let res = stmt.query_one(params![category.name], |row| row.get::<usize, usize>(0))?;

			self.conn.execute("INSERT INTO runs VALUES(NULL, ?1)", params![res])?;

			let run_id = self.conn.last_insert_rowid();

			for (num, &split) in run.splits().unwrap().iter().enumerate() {
				let mult = run.mults().unwrap()[num];
				self.conn.execute(
					"INSERT INTO splits (id, split_num, score, hits, mult, run_id) VALUES(NULL, ?1, ?2, ?3, ?4, ?5)",
					params![num, split, 0, mult, run_id],
				)?;
			}

			Ok::<(), rusqlite::Error>(())
		})() {
			Ok(_) => {
				self.conn.execute("COMMIT", ())?;
				Ok(())
			}
			Err(err) => {
				self.conn.execute("ROLLBACK", ())?;
				Err(err)
			}
		}
	}
	#[must_use]
	pub fn get_pb_run(&self, category: &CategoryManager) -> Result<(Vec<i32>, i32, Gamemode)> {
		let category = category.current();
		let mut statement = self.conn.prepare(include_str!("../sql/pb_splits.sql"))?;
		let rows = statement.query_map(params![category.id], |row| {
			Ok((
				row.get::<usize, i32>(0)?,
				row.get::<usize, i32>(1)?,
				row.get::<usize, i32>(2)?,
				row.get::<usize, Gamemode>(3)?,
			))
		})?;
		let splits: Vec<(i32, i32, i32, Gamemode)> = rows.map(|r| r.unwrap()).collect();

		if splits.len() > 0 {
			let scores: Vec<i32> = splits.iter().map(|s| s.0).collect();
			let hits: Vec<i32> = splits.iter().map(|s| s.1).collect();
			let run_id = splits[0].2;
			let mode = splits[0].3;

			let total = scores.iter().sum();

			Ok((scores, total, mode))
		} else {
			Err(rusqlite::Error::QueryReturnedNoRows)
		}
	}

	/// Get the highest core of each split for the category
	#[must_use]
	pub fn get_gold_splits(&self, category: &CategoryManager) -> Result<Vec<i32>> {
		let mut statement = self.conn.prepare(include_str!("../sql/best_splits.sql"))?;
		statement
			.query_map(params![category.current().id], |rows| rows.get(0))?
			.collect::<Result<Vec<i32>>>()
			.map(|v| {
				if v.len() > 0 {
					Ok(v)
				} else {
					Err(rusqlite::Error::QueryReturnedNoRows)
				}
			})?
	}
}

impl ToSql for Gamemode {
	fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput<'_>> {
		match self {
			Gamemode::GreenOrange => Ok(0u8.into()),
			Gamemode::WhiteVanilla => Ok(1u8.into()),
			Gamemode::BlackOnion => Ok(2u8.into()),
		}
	}
}

impl FromSql for Gamemode {
	fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
		match value {
			ValueRef::Integer(0) => Ok(Self::GreenOrange),
			ValueRef::Integer(1) => Ok(Self::WhiteVanilla),
			ValueRef::Integer(2) => Ok(Self::BlackOnion),
			ValueRef::Integer(i) => Err(rusqlite::types::FromSqlError::OutOfRange(i)),
			_ => Err(rusqlite::types::FromSqlError::InvalidType),
		}
	}
}
