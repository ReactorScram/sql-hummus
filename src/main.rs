use std::{io::Read, process::ExitCode};

use anyhow::{Context as _, Result};
use camino::Utf8PathBuf;
use clap::Parser as _;
use serde_json::json;
use sql_hummus::{Kv, Log};

use crate::cli::{Command, KvCommand, LogCommand};

mod cli;

fn truthy_code() -> ExitCode {
    0.into()
}

fn falsy_code() -> ExitCode {
    1.into()
}

fn err_code() -> ExitCode {
    2.into()
}

fn main() -> ExitCode {
    match inner_main() {
        Ok(true) => truthy_code(),
        Ok(false) => falsy_code(),
        Err(e) => {
            eprintln!("{e}");
            err_code()
        }
    }
}

fn run_kv_command(path: Utf8PathBuf, cmd: KvCommand) -> Result<bool> {
    match cmd {
        KvCommand::ContainsKey { key } => match Kv::new(path)?.contains_key(key)? {
            true => {
                println!("true");
                Ok(true)
            }
            false => {
                println!("false");
                Ok(false)
            }
        },
        KvCommand::Get { key } => match Kv::new(path)?.get(key)? {
            Some(value) => {
                println!("{value}");
                Ok(true)
            }
            None => {
                eprintln!("Key not in KV file");
                Ok(false)
            }
        },
        KvCommand::Insert { key, value } => {
            Kv::new(path)?.insert(key, value)?;
            Ok(true)
        }
        KvCommand::WithPrefix { prefix } => {
            let kv = Kv::new(path)?;
            let cursor = kv.with_prefix(&prefix)?;
            for row in cursor {
                let (k, v) = row?;
                println!("{}", serde_json::to_string(&(k, v))?);
            }
            Ok(true)
        }
    }
}

fn run_log_command(path: Utf8PathBuf, cmd: LogCommand) -> Result<bool> {
    match cmd {
        LogCommand::Get { index } => {
            let index = match index {
                cli::LogIndex::DateRange(..) => todo!(),
                cli::LogIndex::DateScalar(..) => todo!(),
                cli::LogIndex::NumberRange(..) => todo!(),
                cli::LogIndex::NumberScalar(index) => index,
            };
            let Some(line) = Log::new(&path)?.get_by_index(index)? else {
                eprintln!("{path:?} has no element at index {index}");
                return Ok(false);
            };
            println!("{}", serde_json::to_string(&line)?);
            Ok(true)
        }
        LogCommand::Iter => {
            let log = Log::new(path)?;
            let cursor = log.iter()?;
            for row in cursor {
                let line = row?;
                println!("{}", serde_json::to_string(&line)?);
            }
            Ok(true)
        }
        LogCommand::Push { content } => {
            let content = content_or_stdin(content.as_deref())?;
            let index = Log::new(path)?.push(content)?;
            println!("{index}");
            Ok(true)
        }
    }
}

fn inner_main() -> Result<bool> {
    let cli = cli::Cli::try_parse()?;
    match cli.cmd {
        Command::Kv { cmd, path } => run_kv_command(path, cmd),
        Command::Log { cmd, path } => run_log_command(path, cmd),
        Command::Paste { annotation } => {
            let clipboard = std::process::Command::new("wl-paste")
                .output()
                .context("wl-paste failed")?;
            let clipboard = str::from_utf8(clipboard.stdout.as_slice())?;
            let content = json!({ "annotation": annotation, "clipboard": clipboard });

            let (log, path) = Log::open_default().context("Log::open_default() failed")?;
            let index = log.push(serde_json::to_string(&content)?)?;
            let output = json!({ "content": content, "index": index, "path": path });
            println!("{}", serde_json::to_string(&output)?);
            Ok(true)
        }
        Command::Push { content } => {
            let content = content_or_stdin(content.as_deref())?;
            let (log, path) = Log::open_default().context("Log::open_default() failed")?;
            let index = log.push(content)?;
            let output = json!({ "index": index, "path": path });
            println!("{}", serde_json::to_string(&output)?);
            Ok(true)
        }
    }
}

fn content_or_stdin(content: Option<&str>) -> Result<std::borrow::Cow<'_, str>> {
    match content {
        Some(x) => Ok(x.into()),
        None => {
            let mut buf = Vec::with_capacity(1_024 * 1_024);
            std::io::stdin().read_to_end(&mut buf)?;
            Ok(String::from_utf8(buf)?.into())
        }
    }
}
