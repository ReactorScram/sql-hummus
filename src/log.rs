use crate::error::{Cookie as _, Error, Result};
use std::time::SystemTime;

use camino::Utf8PathBuf;
use sql_peas::StatementHandle;

pub struct Log {
    inner: sql_peas::Connection,
    get_handle: StatementHandle,
    insert_handle: StatementHandle,
    iter_handle: StatementHandle,
}

pub struct LogCursor<'a> {
    inner: sql_peas::Cursor<'a>,
}

/// A single log entry within the log database
///
/// Called "Element" to match `Vec`
#[derive(serde::Serialize)]
pub struct Element {
    /// Incrementing index within the file, e.g. 1, 2, 3, 4...
    index: i64,

    /// ULID recorded at some point before the database transaction committed
    ulid: ulid::Ulid,
    value: String,
}

impl Iterator for LogCursor<'_> {
    type Item = Result<Element>;

    fn next(&mut self) -> Option<Self::Item> {
        let row = match self.inner.next()? {
            Err(e) => return Some(Err(e.into())),
            Ok(x) => x,
        };
        let index = row.read(0);
        let ulid = row.read::<&str, _>(1);
        let ulid = match ulid::Ulid::from_string(ulid) {
            Err(e) => return Some(Err(e.into())),
            Ok(x) => x,
        };
        let value = row.read::<&str, _>(2).to_owned();
        Some(Ok(Element { index, ulid, value }))
    }
}

const LOG_USER_VERSION: u32 = 756530437;
const LOG_USER_VERSION_I64: i64 = LOG_USER_VERSION as i64;
const LOG_TABLE_NAME: &str = "sql_hummus_0_log";

impl Log {
    /// Opens the default user-scope log.
    ///
    /// Only use this as a result of user interaction.
    ///
    /// The default log is shared among everything the user does, so it would get spammed easily if automated processes write to it.
    pub fn open_default() -> Result<(Log, Utf8PathBuf)> {
        let dirs = directories::ProjectDirs::from("", "ReactorScram", "sql-hummus")
            .ok_or(Error::from("directories::ProjectDirs failed"))?;
        let dir = dirs.data_local_dir();
        std::fs::create_dir_all(dir).cookie("create_dir_all() for default data dir failed")?;
        let path = dir.join("default-log.db").try_into()?;
        let log = Log::new(&path)?;
        Ok((log, path))
    }

    pub fn new<P: AsRef<std::path::Path>>(p: P) -> Result<Self> {
        let mut inner = sql_peas::Connection::open(p)?;

        // Set some pragmas where new apps need different defaults than SQLite's defaults
        inner.execute("PRAGMA trusted_schema = 0;")?;
        inner.execute("PRAGMA foreign_keys = 1;")?;

        let needs_setup = {
            let handle = inner.prepare("PRAGMA user_version")?;
            let stmt = inner.borrow_statement(handle)?;
            // FIXME: de-dupe single row read up into SQLite
            let mut rows = stmt.iter();
            let row = rows
                .next()
                .ok_or("Expected one row from PRAGMA user_version")??;
            let needs_setup = match row.read(0) {
                0 => true,
                LOG_USER_VERSION_I64 => false,
                _ => {
                    return Err("PRAGMA user_version looks like a non-KV file")?;
                }
            };

            inner.drop_statement(handle)?;
            needs_setup
        };

        if needs_setup {
            // FIXME: On further reflection, the logs could be implemented as a layer on top of the key-value store, if I combine features a little bit and decide that storing local time is the user's business, which is probably right.
            // But I'll keep the original design for now.

            inner.execute(format!("CREATE TABLE IF NOT EXISTS {LOG_TABLE_NAME} (idx INTEGER PRIMARY KEY NOT NULL, ulid TEXT NOT NULL UNIQUE, value TEXT)"))?;
            inner.execute(format!("PRAGMA user_version = {LOG_USER_VERSION}"))?;
        }

        let get_handle = inner.prepare(format!(
            "SELECT idx, ulid, value from {LOG_TABLE_NAME} WHERE idx = ?"
        ))?;
        let insert_handle = inner.prepare(format!(
            "INSERT INTO {LOG_TABLE_NAME} (ulid, value) VALUES (?, ?) RETURNING idx"
        ))?;
        let iter_handle = inner.prepare(format!(
            "SELECT idx, ulid, value FROM {LOG_TABLE_NAME} ORDER BY ulid"
        ))?;

        Ok(Self {
            inner,
            get_handle,
            insert_handle,
            iter_handle,
        })
    }

    pub fn clear(&self) -> Result<()> {
        self.inner
            .execute(format!("DELETE FROM {LOG_TABLE_NAME}"))?;
        Ok(())
    }

    pub fn get_by_index(&self, index: i64) -> Result<Option<Element>> {
        let stmt = self.inner.borrow_statement(self.get_handle)?;
        stmt.bind((1, index))?;
        if stmt.next()? != sql_peas::State::Row {
            return Ok(None);
        }
        if index != stmt.read(0)? {
            Err("Log::get didn't get the same index back from the DB")?;
        }
        let ulid = ulid::Ulid::from_string(&stmt.read::<String, _>(1)?)?;
        let value = stmt.read(2)?;
        Ok(Some(Element { index, ulid, value }))
    }

    pub fn iter<'a>(&'a self) -> Result<LogCursor<'a>> {
        let stmt = self.inner.borrow_statement(self.iter_handle)?;
        let cursor = stmt.iter();
        Ok(LogCursor { inner: cursor })
    }

    pub(crate) fn push_inner(&self, ts: &str, value: &str) -> Result<i64> {
        let stmt = self.inner.borrow_statement(self.insert_handle).cookie(
            "KPQL7L2G
        ",
        )?;
        stmt.bind((1, ts)).cookie("7RMQCY4N")?;
        stmt.bind((2, value)).cookie("AGRZOZBD")?;
        if stmt.next().cookie("WEMNOGO7")? != sql_peas::State::Row {
            Err("We didn't get State::Row during Log::insert")?;
        }
        let index = stmt.read(0).cookie("MEJYJBDW")?;
        if stmt.next().cookie("QOV7GD47")? != sql_peas::State::Done {
            Err("We didn't get State::Done during Log::insert")?;
        }
        Ok(index)
    }

    /// Insert with user-specified time in case the user does store their local time inside the line.
    pub fn push_with_time<S: AsRef<str>>(&self, ts: SystemTime, value: S) -> Result<i64> {
        let ulid = ulid::Ulid::from_datetime(ts);
        self.push_inner(ulid.to_string().as_str(), value.as_ref())
    }

    pub fn push<S: AsRef<str>>(&self, value: S) -> Result<i64> {
        let ulid = ulid::Ulid::generate();
        self.push_inner(ulid.to_string().as_str(), value.as_ref())
            .cookie("6JEUVGMI")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() -> Result<()> {
        let path = "sql_hummus_test_temp_IVTIQYBF.db";

        let cxn = Log::new(path).unwrap();
        cxn.clear()?;

        assert_eq!(cxn.push_inner("01KX54X6X3G98K9A24P1HBA8GY", "test 1")?, 1);
        assert_eq!(cxn.push_inner("01KX54X8MW2N2FA4ERGGXVNTWH", "test 2")?, 2);

        let mut cursor = cxn.iter()?;

        let line = cursor.next().unwrap().unwrap();
        assert_eq!(line.index, 1);
        assert_eq!(line.ulid.to_string(), "01KX54X6X3G98K9A24P1HBA8GY");
        assert_eq!(line.value, "test 1");

        let line = cursor.next().unwrap().unwrap();
        assert_eq!(line.index, 2);
        assert_eq!(line.ulid.to_string(), "01KX54X8MW2N2FA4ERGGXVNTWH");
        assert_eq!(line.value, "test 2");

        Ok(())
    }
}
