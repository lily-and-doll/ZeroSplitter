use std::sync::Arc;

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

macro_rules! transaction {
	($conn:expr, $inner:expr) => {{
		$conn.execute("BEGIN TRANSACTION", ())?;

		match (|| {
			{
				$inner
			}

			Ok::<(), rusqlite::Error>(())
		})() {
			Ok(_) => {
				$conn.execute("COMMIT", ())?;
				Ok(())
			}
			Err(err) => {
				$conn.execute("ROLLBACK", ())?;
				Err(err)
			}
		}
	}};
}

const CURRENT_SCHEMA_VERSION: i32 = 4;

impl Database {
	pub fn init() -> Result<Self> {
		let database = Database {
			conn: Arc::new(Connection::open("./sqlite.db3")?),
		};

		// create tables if they don't exist
		if !database.conn.table_exists(Some("main"), "categories")? {
			if let Err(err) = database.create_tables0() {
				error!("Error creating tables: {}", err)
			};
			database.insert_new_category("default".to_owned(), Gamemode::GreenOrange)?;
		}
		// check version of schema (0 if they were just created)
		let schema_version = database
			.conn
			.query_one("PRAGMA user_version", (), |row| row.get::<_, i32>(0))
			.unwrap();

		// migrate schema version if necessary
		if schema_version < CURRENT_SCHEMA_VERSION {
			database.migrate(schema_version)
		} else if schema_version > CURRENT_SCHEMA_VERSION {
			panic!(
				"Schema version beyond program version!\n Schema version {schema_version}, program version {CURRENT_SCHEMA_VERSION}"
			)
		}

		Ok(database)
	}
	pub fn create_tables0(&self) -> Result<()> {
		self.conn.execute("BEGIN TRANSACTION", ())?;

		match (|| {
			self.conn.pragma_update(Some("main"), "user_version", 0)?;
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
        mult        INTEGER,
        run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE
    )
    ",
				(),
			)?;

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

	pub fn insert_new_category(&self, name: String, mode: Gamemode) -> Result<i64> {
		self.conn
			.execute("INSERT INTO categories VALUES(NULL, ?1, ?2)", params![name, mode])?;

		Ok(self.conn.last_insert_rowid())
	}

	pub fn delete_category(&self, category: Category) -> Result<usize> {
		self.conn
			.execute("DELETE FROM categories WHERE id = ?1", params![category.id])
	}

	pub fn rename_category(&self, category: &Category, new_name: String) -> Result<usize> {
		self.conn.execute(
			"UPDATE categories SET name='?1' WHERE id=?2",
			params![new_name, category.id],
		)
	}
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

	pub fn insert_run(&self, category: &CategoryManager, run: &Run) -> Result<()> {
		let category = category.current();
		self.conn.execute("BEGIN TRANSACTION", ())?;

		match (|| {
			let mut stmt = self.conn.prepare("SELECT id FROM categories WHERE name = ?1")?;
			let res = stmt.query_one(params![category.name], |row| row.get::<usize, usize>(0))?;

			self.conn.execute(
				"INSERT INTO runs (id, category, datetime, imported) VALUES(NULL, ?1, datetime('now'), false)",
				params![res],
			)?;

			let run_id = self.conn.last_insert_rowid();

			for (num, &split) in run.splits().unwrap().iter().take_while(|&&s| s.score > 0).enumerate() {
				let final_split = num == run.current_split().unwrap();
				self.conn.execute(
					"INSERT INTO splits (id, split_num, score, hits, mult, run_id, final, pattern_rank, dynamic_rank) VALUES(NULL, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
					params![num, split.score, 0, split.mult, run_id, final_split, split.pattern_rank, split.dynamic_rank],
				)?;
			}

			Ok::<(), rusqlite::Error>(())
		})() {
			Ok(_) => {
				self.conn.execute("COMMIT", ())?;
				println!("Committing run with score {} to database", run.score().unwrap());
				Ok(())
			}
			Err(err) => {
				self.conn.execute("ROLLBACK", ())?;
				Err(err)
			}
		}
	}

	pub fn get_pb_run(&self, category: &CategoryManager) -> Result<(Vec<i32>, i32, Gamemode)> {
		let category = category.current();
		let mut statement = self.conn.prepare(include_str!("../sql/pb_splits.sql"))?;
		let rows = statement.query_map(params![category.id], |row| {
			Ok((
				row.get::<usize, i32>(0)?, //score
				row.get::<usize, Option<i32>>(1)?, //mult
				row.get::<usize, i32>(2)?, //run_id
				row.get::<usize, Gamemode>(3)?,
			))
		})?;
		let splits: Vec<(i32, Option<i32>, i32, Gamemode)> = rows.map(|r| r.unwrap()).collect();

		if splits.len() > 0 {
			let scores: Vec<i32> = splits.iter().map(|s| s.0).collect();
			let _hits: Vec<i32> = splits.iter().map(|s| s.1.unwrap_or(0)).collect();
			let _run_id = splits[0].2;
			let mode = splits[0].3;

			let total = scores.iter().sum();

			Ok((scores, total, mode))
		} else {
			Err(rusqlite::Error::QueryReturnedNoRows)
		}
	}

	/// Get the highest core of each split for the category
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

	fn migrate(&self, schema_version: i32) {
		println!("Migrating database from {schema_version} to {CURRENT_SCHEMA_VERSION}");

		self.conn.execute("BEGIN TRANSACTION", ()).unwrap();

		let mut current_schema = schema_version;

		match (|| {
			while current_schema < CURRENT_SCHEMA_VERSION {
				match current_schema {
					0 => {
						self.migrate0to1()?;
						current_schema = 1
					}
					1 => {
						self.migrate1to2()?;
						current_schema = 2
					}
					2 => {
						self.migrate2to3()?;
						current_schema = 3
					}
					3 => {
						self.migrate3to4()?;
						current_schema = 4
					}
					_ => Err(rusqlite::Error::InvalidQuery)?,
				};
			}
			Ok::<(), rusqlite::Error>(())
		})() {
			Ok(_) => {
				self.conn.execute("COMMIT", ()).unwrap();
				println!("Migration successful")
			}
			Err(err) => {
				self.conn.execute("ROLLBACK", ()).unwrap();
				panic!("Migration failed! {err}")
			}
		}
	}

	fn migrate0to1(&self) -> Result<usize> {
		println!("Migrating schema 0 to 1...");
		self.conn.pragma_update(Some("main"), "user_version", 1)?;
		self.conn.execute("ALTER TABLE splits ADD COLUMN final BOOLEAN", ())
	}

	fn migrate1to2(&self) -> Result<usize> {
		println!("Migrating schema 1 to 2...");
		self.conn.pragma_update(Some("main"), "user_version", 2)?;
		self.conn.execute("ALTER TABLE runs ADD COLUMN datetime INTEGER", ())
	}

	fn migrate2to3(&self) -> Result<()> {
		println!("Migrating schema 2 to 3...");
		self.conn.pragma_update(Some("main"), "user_version", 3)?;
		self.conn.execute_batch(
			"ALTER TABLE splits ADD COLUMN pattern_rank REAL;
			ALTER TABLE splits ADD COLUMN dynamic_rank REAL;",
		)
	}

	fn migrate3to4(&self) -> Result<usize> {
		println!("Migrating schema 3 to 4...");
		self.conn.pragma_update(Some("main"), "user_version", 4)?;
		self.conn.execute("ALTER TABLE runs ADD COLUMN imported BOOLEAN", ())
	}

	pub fn import_run(&self, splits: Vec<i32>, category_name: &String) -> Result<()> {
		transaction!(self.conn, {
			let category_id =
				self.conn
					.query_one("SELECT id FROM categories WHERE name=?1", params![category_name], |r| {
						r.get::<_, i32>(0)
					})?;
			self.conn
				.execute(
					"INSERT INTO runs (category, imported) VALUES (?1, true)",
					params![category_id],
				)
				.unwrap();
			let last = self.conn.last_insert_rowid();
			for (idx, score) in splits.iter().enumerate() {
				self.conn
					.execute(
						"INSERT INTO splits (split_num, score, run_id) VALUES (?1, ?2, ?3)",
						params![idx as i32, score, last],
					)
					.unwrap();
			}
		})
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
