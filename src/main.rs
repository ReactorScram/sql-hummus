use std::{ops::Range, str::FromStr};

use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use chrono::DateTime;
use clap::Parser as _;

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
}
