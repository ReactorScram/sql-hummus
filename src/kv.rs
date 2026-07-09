use sqlite::StatementHandle;

pub struct Kv {
    inner: sqlite::Connection,
    contains_handle: StatementHandle,
    get_handle: StatementHandle,
    insert_handle: StatementHandle,
    prefix_handle: StatementHandle,
}

pub struct KvCursor<'a> {
    inner: sqlite::Cursor<'a>,
    prefix: &'a str,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] sqlite::Error),
    #[error("Error: {0}")]
    Other(&'static str),
}

type Result<T> = std::result::Result<T, Error>;

impl Iterator for KvCursor<'_> {
    type Item = Result<(String, String)>;

    fn next(&mut self) -> Option<Self::Item> {
        let row = match self.inner.next()? {
            Err(e) => return Some(Err(e.into())),
            Ok(x) => x,
        };
        let key = row.read::<&str, _>(0);
        if !key.starts_with(self.prefix) {
            return None;
        }
        let value = row.read::<&str, _>(1);
        Some(Ok((key.into(), value.into())))
    }
}

const KV_USER_VERSION: u32 = 1783402338;
const KV_USER_VERSION_I64: i64 = KV_USER_VERSION as i64;
const KV_TABLE_NAME: &str = "sql_hummus_0_kv";

impl Kv {
    pub fn new<P: AsRef<std::path::Path>>(p: P) -> Result<Self> {
        let mut inner = sqlite::Connection::open(p)?;

        // Set some pragmas where new apps need different defaults than SQLite's defaults
        inner.execute("PRAGMA trusted_schema = 0;")?;
        inner.execute("PRAGMA foreign_keys = 1;")?;

        let needs_setup = {
            let handle = inner.prepare("PRAGMA user_version")?;
            let stmt = inner.borrow_statement(handle)?;
            let mut rows = stmt.iter();
            let row = rows
                .next()
                .ok_or(Error::Other("Expected one row from PRAGMA user_version"))??;
            let needs_setup = match row.read(0) {
                0 => true,
                KV_USER_VERSION_I64 => false,
                _ => return Err(Error::Other("PRAGMA user_version looks like a non-KV file")),
            };
            inner.drop_statement(handle)?;
            needs_setup
        };

        if needs_setup {
            // Looks like a new file, so let's set everything up and then stamp it

            inner.execute(format!("CREATE TABLE IF NOT EXISTS {KV_TABLE_NAME} (key TEXT PRIMARY KEY NOT NULL, value TEXT) WITHOUT ROWID;"))?;

            // Using `format!()` here because PRAGMA doesn't seem to like bound params.
            // SECURITY: KV_USER_VERSION is not user-controlled input, it's a compile-time const that we control, so it's not a SQL injection vuln.
            inner.execute(format!("PRAGMA user_version = {KV_USER_VERSION}"))?;
        }

        let contains_handle =
            inner.prepare(format!("SELECT count() FROM {KV_TABLE_NAME} WHERE key = ?"))?;
        let get_handle =
            inner.prepare(format!("SELECT value FROM {KV_TABLE_NAME} WHERE key = ?"))?;
        let insert_handle = inner.prepare(format!("INSERT INTO {KV_TABLE_NAME} (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"))?;
        let prefix_handle = inner.prepare(format!(
            "SELECT key, value FROM {KV_TABLE_NAME} WHERE key >= ? ORDER BY key"
        ))?;

        Ok(Self {
            inner,
            contains_handle,
            get_handle,
            insert_handle,
            prefix_handle,
        })
    }

    pub fn clear(&self) -> Result<()> {
        self.inner.execute(format!("DELETE FROM {KV_TABLE_NAME}"))?;
        Ok(())
    }

    pub fn contains_key<S: AsRef<str>>(&self, key: S) -> Result<bool> {
        let stmt = self.inner.borrow_statement(self.contains_handle)?;
        stmt.bind((1, key.as_ref()))?;
        let mut rows = stmt.iter();
        let row = rows.next().ok_or(Error::Other("Expected one row here"))??;
        match row.read::<i64, _>(0) {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Other(
                "Logic error - Two rows in KV table have the same key",
            )),
        }
    }

    pub fn get<S: AsRef<str>>(&self, key: S) -> Result<Option<String>> {
        let stmt = self.inner.borrow_statement(self.get_handle)?;
        stmt.bind((1, key.as_ref()))?;
        let mut rows = stmt.iter();
        match rows.next() {
            None => Ok(None),
            Some(Ok(row)) => Ok(Some(row.read::<&str, _>(0).into())),
            Some(Err(e)) => Err(e)?,
        }
    }

    pub fn insert<S: AsRef<str>, T: AsRef<str>>(&self, key: S, value: T) -> Result<Option<String>> {
        let tx = self.inner.begin_immediate_transaction()?;
        let old_value = self.get(&key)?;
        {
            let stmt = self.inner.borrow_statement(self.insert_handle)?;
            stmt.bind((1, key.as_ref()))?;
            stmt.bind((2, value.as_ref()))?;
            if stmt.next()? != sqlite::State::Done {
                return Err(Error::Other(
                    "We didn't get sqlite::State::Done during Kv::insert",
                ));
            }
        }
        tx.commit()?;
        Ok(old_value)
    }

    pub fn with_prefix<'a>(&'a self, prefix: &'a str) -> Result<KvCursor<'a>> {
        let stmt = self.inner.borrow_statement(self.prefix_handle)?;
        stmt.bind((1, prefix))?;
        let cursor = stmt.iter();
        Ok(KvCursor {
            inner: cursor,
            prefix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv() -> Result<()> {
        let path = "sql_hummus_test_temp_ZQ3KECAY.db";
        const COLOR_MODE: &str = "/myapp/color_mode";

        let cxn = Kv::new(path).unwrap();
        cxn.clear()?;
        assert!(!cxn.contains_key(COLOR_MODE).unwrap());

        assert!(cxn.insert(COLOR_MODE, "light_mode").unwrap().is_none());
        assert_eq!(cxn.get(COLOR_MODE).unwrap(), Some("light_mode".into()));
        assert_eq!(
            cxn.insert(COLOR_MODE, "dark_mode").unwrap(),
            Some("light_mode".into())
        );
        assert_eq!(
            cxn.insert(COLOR_MODE, "dark_mode").unwrap(),
            Some("dark_mode".into())
        );
        assert_eq!(cxn.get(COLOR_MODE)?, Some("dark_mode".into()));

        assert!(cxn.contains_key(COLOR_MODE)?);
        assert!(!cxn.contains_key("Never used this key at all")?);

        cxn.insert("/myapp/bookmarks/https://example.com/", "")?;
        cxn.insert("/aaapppp/", "")?;
        cxn.insert("/notmyapp/bookmarks/", "asdf")?;

        assert_eq!(
            cxn.with_prefix("/myapp/")?.collect::<Result<Vec<_>>>()?,
            [
                ("/myapp/bookmarks/https://example.com/", ""),
                (COLOR_MODE, "dark_mode"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        );

        assert_eq!(
            cxn.with_prefix("")?.collect::<Result<Vec<_>>>()?,
            [
                ("/aaapppp/", ""),
                ("/myapp/bookmarks/https://example.com/", ""),
                (COLOR_MODE, "dark_mode"),
                ("/notmyapp/bookmarks/", "asdf"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        );

        Ok(())
    }
}
