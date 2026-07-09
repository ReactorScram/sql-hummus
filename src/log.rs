use std::time::SystemTime;

use sqlite::StatementHandle;

pub struct Log {
    inner: sqlite::Connection,
    insert_handle: StatementHandle,
}

pub struct LogCursor<'a> {
    inner: sqlite::Cursor<'a>,
}

#[derive(serde::Serialize)]
pub struct LogLine {
    ulid: ulid::Ulid,
    value: String,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] sqlite::Error),
    #[error("Error: {0}")]
    Other(&'static str),
    #[error("ULID decode error: {0}")]
    UlidDecode(#[from] ulid::DecodeError),
}

type Result<T> = std::result::Result<T, Error>;

impl Iterator for LogCursor<'_> {
    type Item = Result<LogLine>;

    fn next(&mut self) -> Option<Self::Item> {
        let row = match self.inner.next()? {
            Err(e) => return Some(Err(e.into())),
            Ok(x) => x,
        };
        let ulid = row.read::<&str, _>(0);
        let ulid = match ulid::Ulid::from_string(ulid) {
            Err(e) => return Some(Err(e.into())),
            Ok(x) => x,
        };
        let value = row.read::<&str, _>(1).to_owned();
        Some(Ok(LogLine { ulid, value }))
    }
}

const LOG_USER_VERSION: u32 = 756530437;
const LOG_USER_VERSION_I64: i64 = LOG_USER_VERSION as i64;
const LOG_TABLE_NAME: &str = "sql_hummus_0_log";

impl Log {
    pub fn new<P: AsRef<std::path::Path>>(p: P) -> Result<Self> {
        let mut inner = sqlite::Connection::open(p)?;

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
                .ok_or(Error::Other("Expected one row from PRAGMA user_version"))??;
            let needs_setup = match row.read(0) {
                0 => true,
                LOG_USER_VERSION_I64 => false,
                _ => return Err(Error::Other("PRAGMA user_version looks like a non-KV file")),
            };

            inner.drop_statement(handle)?;
            needs_setup
        };

        if needs_setup {
            // FIXME: On further reflection, the logs could be implemented as a layer on top of the key-value store, if I combine features a little bit and decide that storing local time is the user's business, which is probably right.
            // But I'll keep the original design for now.

            inner.execute(format!("CREATE TABLE IF NOT EXISTS {LOG_TABLE_NAME} (ulid TEXT PRIMARY KEY NOT NULL, value TEXT) WITHOUT ROWID"))?;
            inner.execute(format!("PRAGMA user_version = {LOG_USER_VERSION}"))?;
        }

        let insert_handle = inner.prepare(format!(
            "INSERT INTO {LOG_TABLE_NAME} (ulid, value) VALUES (?, ?)"
        ))?;

        Ok(Self {
            inner,
            insert_handle,
        })
    }

    pub fn clear(&self) -> Result<()> {
        self.inner
            .execute(format!("DELETE FROM {LOG_TABLE_NAME}"))?;
        Ok(())
    }

    fn push_inner(&self, ts: &str, value: &str) -> Result<()> {
        let stmt = self.inner.borrow_statement(self.insert_handle)?;
        stmt.bind((1, ts))?;
        stmt.bind((2, value))?;
        if stmt.next()? != sqlite::State::Done {
            return Err(Error::Other(
                "We didn't get sqlite::State::Done during Log::insert",
            ));
        }
        Ok(())
    }

    /// Insert with user-specified time in case the user does store their local time inside the line.
    pub fn push_with_time<S: AsRef<str>>(&self, ts: SystemTime, value: S) -> Result<()> {
        let ulid = ulid::Ulid::from_datetime(ts);
        self.push_inner(ulid.to_string().as_str(), value.as_ref())
    }

    pub fn push<S: AsRef<str>>(&self, value: S) -> Result<()> {
        let ulid = ulid::Ulid::new();
        self.push_inner(ulid.to_string().as_str(), value.as_ref())
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

        cxn.push("test")?;
        Ok(())
    }
}
