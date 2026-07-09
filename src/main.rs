use std::{ops::Range, path::Path, str::FromStr};

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use chrono::DateTime;
use clap::Parser as _;
use rusqlite::Connection;

#[derive(clap::Parser, Debug, PartialEq, Eq)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum Command {
    /// Key-value files
    Kv {
        #[command(subcommand)]
        cmd: KvCommand,
    },

    /// Log files
    Log {
        #[command(subcommand)]
        cmd: LogCommand,
    },
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum KvCommand {
    ContainsKey {
        /// Path of the database file
        ///
        /// e.g. "log.db" or "kv.db"
        path: Utf8PathBuf,
        key: String,
    },
    Get {
        /// Path of the database file
        ///
        /// e.g. "log.db" or "kv.db"
        path: Utf8PathBuf,
        key: String,
    },
    Insert {
        path: Utf8PathBuf,
        key: String,
        value: String,
    },
    WithPrefix {
        path: Utf8PathBuf,
        prefix: String,
    },
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum LogCommand {
    Get {
        /// Path of the database file
        ///
        /// e.g. "log.db" or "kv.db"
        path: Utf8PathBuf,
        index: LogIndex,
    },
    Iter,
    /// Writes a new log element to a log database file
    Push {
        /// Path of the database file
        ///
        /// e.g. "log.db"
        path: Utf8PathBuf,

        /// Content to append (or read from stdin if none provided)
        content: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LogIndex {
    DateRange(Range<DateTime<chrono::FixedOffset>>),
    DateScalar(DateTime<chrono::FixedOffset>),
    NumberRange(Range<u64>),
    NumberScalar(u64),
}

impl FromStr for LogIndex {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(dots_pos) = s.find("..") {
            ensure!(s.is_ascii());
            let start = &s[0..dots_pos];
            let end = &s[dots_pos + 2..];

            {
                let start = u64::from_str(start);
                let end = u64::from_str(end);
                if let (Ok(start), Ok(end)) = (start, end) {
                    return Ok(Self::NumberRange(start..end));
                }
            }

            Ok(Self::DateRange(
                DateTime::parse_from_rfc3339(start)?..DateTime::parse_from_rfc3339(end)?,
            ))
        } else {
            if let Ok(x) = u64::from_str(s) {
                Ok(Self::NumberScalar(x))
            } else {
                Ok(Self::DateScalar(DateTime::parse_from_rfc3339(s)?))
            }
        }
    }
}

pub struct Kv {
    inner: rusqlite::Connection,
}

pub struct KvCursor<'a> {

}

impl Iterator for KvCursor<'_> {
    type Item = Result<(String, String)>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

const KV_USER_VERSION: u32 = 1783402338;
const KV_USER_VERSION_I64: i64 = KV_USER_VERSION as i64;

impl Kv {
    pub fn new<P: AsRef<Path>>(p: P) -> Result<Self> {
        let inner = rusqlite::Connection::open(p)?;

        // Set some pragmas where new apps need different defaults than SQLite's defaults
        inner.execute("PRAGMA trusted_schema = 0;", [])?;
        inner.execute("PRAGMA foreign_keys = 1;", [])?;

        {
            let mut stmt = inner.prepare("PRAGMA user_version")?;
            let mut rows = stmt.query([])?;
            let row = rows.next()?.context("Expected one row")?;
            match row.get::<_, i64>(0)? {
                0 => {}
                KV_USER_VERSION_I64 => {
                    drop(rows);
                    drop(stmt);
                    return Ok(Self { inner });
                }
                _ => bail!("PRAGMA user_version looks like a non-KV file"),
            }
        }

        // Looks like a new file, so let's set everything up and then stamp it

        inner.execute("CREATE TABLE IF NOT EXISTS sql_appetizer_0_kv (key TEXT PRIMARY KEY NOT NULL, value TEXT) WITHOUT ROWID;", [])?;

        // Using `format!()` here because PRAGMA doesn't seem to like bound params.
        // SECURITY: KV_USER_VERSION is not user-controlled input, it's a compile-time const that we control, so it's not a SQL injection vuln.
        inner.execute(&format!("PRAGMA user_version = {KV_USER_VERSION}"), [])?;
        Ok(Self { inner })
    }

    pub fn clear(&self) -> Result<()> {
        self.inner.execute("DELETE FROM sql_appetizer_0_kv;", [])?;
        Ok(())
    }

    pub fn contains_key<S: AsRef<str>>(&self, key: S) -> Result<bool> {
        let mut stmt = self
            .inner
            .prepare_cached("SELECT count() FROM sql_appetizer_0_kv WHERE key = ?;")?;
        let mut rows = stmt.query(&[key.as_ref()])?;
        let row = rows.next()?.context("Expected one row here")?;
        match row.get(0)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => bail!("Logic error - Two rows in KV table have the same key"),
        }
    }

    fn get_inner<S: AsRef<str>>(cxn: &Connection, key: S) -> Result<Option<String>> {
        let mut stmt = cxn.prepare_cached("SELECT value FROM sql_appetizer_0_kv WHERE key = ?;")?;
        let mut rows = stmt.query(&[key.as_ref()])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => Ok(Some(row.get(0)?)),
        }
    }

    pub fn get<S: AsRef<str>>(&self, key: S) -> Result<Option<String>> {
        Self::get_inner(&self.inner, key)
    }

    pub fn insert<S: AsRef<str>, T: AsRef<str>>(
        &mut self,
        key: S,
        value: T,
    ) -> Result<Option<String>> {
        let tx = self
            .inner
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let old_value = Self::get_inner(&tx, &key)?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT INTO sql_appetizer_0_kv (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?;
            let num_changes = stmt.execute(&[key.as_ref(), value.as_ref()])?;
            ensure!(num_changes == 1);
        }
        tx.commit()?;
        Ok(old_value)
    }

    pub fn with_prefix<S: AsRef<str>>(&self, prefix: S) -> Result<KvCursor<'_>> {
        todo!()
    }
}

fn main() -> Result<()> {
    let _cli = Cli::try_parse()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_cli(input: &[&str], expected: Cli) {
        let actual = Cli::parse_from(input);
        assert_eq!(actual, expected);
    }

    #[test]
    fn cli() {
        check_cli(
            &[
                "app_name",
                "kv",
                "insert",
                "kv.db",
                "/myapp/bookmarks/https://example.com",
                "",
            ],
            Cli {
                cmd: Command::Kv {
                    cmd: KvCommand::Insert {
                        path: "kv.db".into(),
                        key: "/myapp/bookmarks/https://example.com".into(),
                        value: "".into(),
                    },
                },
            },
        );

        check_cli(
            &[
                "app_name",
                "log",
                "push",
                "log.db",
                "Writing a new element to a log file",
            ],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Push {
                        path: "log.db".into(),
                        content: Some("Writing a new element to a log file".into()),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "push", "log.db", ""],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Push {
                        path: "log.db".into(),
                        content: Some("".into()),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "push", "log.db"],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Push {
                        path: "log.db".into(),
                        content: None,
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "get", "log.db", "0"],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Get {
                        path: "log.db".into(),
                        index: LogIndex::NumberScalar(0),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "get", "log.db", "0..2"],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Get {
                        path: "log.db".into(),
                        index: LogIndex::NumberRange(0..2),
                    },
                },
            },
        );

        check_cli(
            &[
                "app_name",
                "log",
                "get",
                "log.db",
                "2026-07-07 04:31:00+00:00",
            ],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Get {
                        path: "log.db".into(),
                        index: LogIndex::DateScalar(
                            DateTime::parse_from_rfc3339("2026-07-07 04:31:00+00:00").unwrap(),
                        ),
                    },
                },
            },
        );

        check_cli(
            &[
                "app_name",
                "log",
                "get",
                "log.db",
                "2026-07-07 04:31:00+00:00..2026-07-07 04:32:00+00:00",
            ],
            Cli {
                cmd: Command::Log {
                    cmd: LogCommand::Get {
                        path: "log.db".into(),
                        index: LogIndex::DateRange(
                            DateTime::parse_from_rfc3339("2026-07-07 04:31:00+00:00").unwrap()
                                ..DateTime::parse_from_rfc3339("2026-07-07 04:32:00+00:00")
                                    .unwrap(),
                        ),
                    },
                },
            },
        );
    }

    #[test]
    fn kv_mode() -> Result<()> {
        let path = "sql_appetizer_test_temp_ZQ3KECAY.db";
        const COLOR_MODE: &str = "/myapp/color_mode";

        let mut cxn = Kv::new(path)?;
        cxn.clear()?;
        assert!(!cxn.contains_key(COLOR_MODE)?);

        assert!(cxn.insert(COLOR_MODE, "light_mode")?.is_none());
        assert_eq!(cxn.get(COLOR_MODE)?, Some("light_mode".into()));
        assert_eq!(
            cxn.insert(COLOR_MODE, "dark_mode")?,
            Some("light_mode".into())
        );
        assert_eq!(
            cxn.insert(COLOR_MODE, "dark_mode")?,
            Some("dark_mode".into())
        );
        assert_eq!(cxn.get(COLOR_MODE)?, Some("dark_mode".into()));

        assert!(cxn.contains_key(COLOR_MODE)?);
        assert!(!cxn.contains_key("Never used this key at all")?);

        cxn.insert("/myapp/bookmarks/https://example.com/", "")?;
        cxn.insert("/aaapppp/", "")?;
        cxn.insert("/notmyapp/bookmarks/", "asdf")?;

        assert_eq!(
            cxn.with_prefix("/myapp/")?.collect::<Result<Vec<_>, _>>()?,
            [
                (COLOR_MODE, "dark_mode"),
                ("/myapp/bookmarks/https://example.com/", ""),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        );

        assert_eq!(
            cxn.with_prefix("")?.collect::<Result<Vec<_>, _>>()?,
            [
                ("/aaapppp/", ""),
                (COLOR_MODE, "dark_mode"),
                ("/myapp/bookmarks/https://example.com/", ""),
                ("/notmyapp/bookmarks/", "asdf"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        );

        cxn.clear()?;
        assert!(!cxn.contains_key(COLOR_MODE)?);

        Ok(())
    }
}
