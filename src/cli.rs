use std::{ops::Range, str::FromStr};

use anyhow::ensure;
use camino::Utf8PathBuf;
use chrono::DateTime;

#[derive(clap::Parser, Debug, PartialEq, Eq)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) cmd: Command,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Key-value files
    Kv {
        /// Path of the key-value file
        ///
        /// e.g. "kv.db"
        path: Utf8PathBuf,
        #[command(subcommand)]
        cmd: KvCommand,
    },

    /// Log files
    Log {
        /// Path of the log file
        ///
        /// e.g. "log.db"
        path: Utf8PathBuf,
        #[command(subcommand)]
        cmd: LogCommand,
    },

    /// Quick push to the default log file
    Push { content: Option<String> },
}

/// Commands that operate on a key-value database file
#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
pub(crate) enum KvCommand {
    ContainsKey {
        key: String,
    },
    Get {
        key: String,
    },
    Insert {
        key: String,
        value: String,
    },
    WithPrefix {
        #[arg(default_value_t = String::new())]
        prefix: String,
    },
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
pub(crate) enum LogCommand {
    Get {
        index: LogIndex,
    },
    Iter,
    /// Writes a new log element to a log database file
    Push {
        /// Content to append (If no content is provided, sql-hummus reads stdin to a memory buffer and appends that)
        content: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LogIndex {
    DateRange(Range<DateTime<chrono::FixedOffset>>),
    DateScalar(DateTime<chrono::FixedOffset>),
    NumberRange(Range<i64>),
    NumberScalar(i64),
}

impl FromStr for LogIndex {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(dots_pos) = s.find("..") {
            ensure!(s.is_ascii());
            let start = &s[0..dots_pos];
            let end = &s[dots_pos + 2..];

            {
                let start = i64::from_str(start);
                let end = i64::from_str(end);
                if let (Ok(start), Ok(end)) = (start, end) {
                    return Ok(Self::NumberRange(start..end));
                }
            }

            Ok(Self::DateRange(
                DateTime::parse_from_rfc3339(start)?..DateTime::parse_from_rfc3339(end)?,
            ))
        } else {
            if let Ok(x) = i64::from_str(s) {
                Ok(Self::NumberScalar(x))
            } else {
                Ok(Self::DateScalar(DateTime::parse_from_rfc3339(s)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

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
                "kv.db",
                "insert",
                "/myapp/bookmarks/https://example.com",
                "",
            ],
            Cli {
                cmd: Command::Kv {
                    path: "kv.db".into(),
                    cmd: KvCommand::Insert {
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
                "log.db",
                "push",
                "Writing a new element to a log file",
            ],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Push {
                        content: Some("Writing a new element to a log file".into()),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "log.db", "push", ""],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Push {
                        content: Some("".into()),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "log.db", "push"],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Push { content: None },
                },
            },
        );

        check_cli(
            &["app_name", "log", "log.db", "get", "0"],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Get {
                        index: LogIndex::NumberScalar(0),
                    },
                },
            },
        );

        check_cli(
            &["app_name", "log", "log.db", "get", "0..2"],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Get {
                        index: LogIndex::NumberRange(0..2),
                    },
                },
            },
        );

        check_cli(
            &[
                "app_name",
                "log",
                "log.db",
                "get",
                "2026-07-07 04:31:00+00:00",
            ],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Get {
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
                "log.db",
                "get",
                "2026-07-07 04:31:00+00:00..2026-07-07 04:32:00+00:00",
            ],
            Cli {
                cmd: Command::Log {
                    path: "log.db".into(),
                    cmd: LogCommand::Get {
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
