use std::process::ExitCode;

use anyhow::Result;
use clap::Parser as _;
use sql_hummus::Kv;

use crate::cli::{Command, KvCommand};

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

fn inner_main() -> Result<bool> {
    let cli = cli::Cli::try_parse()?;
    match cli.cmd {
        Command::Kv { cmd } => match cmd {
            KvCommand::ContainsKey { path, key } => match Kv::new(path)?.contains_key(key)? {
                true => {
                    println!("true");
                    Ok(true)
                }
                false => {
                    println!("false");
                    Ok(false)
                }
            },
            KvCommand::Get { path, key } => match Kv::new(path)?.get(key)? {
                Some(value) => {
                    println!("{value}");
                    Ok(true)
                }
                None => {
                    eprintln!("Key not in KV file");
                    Ok(false)
                }
            },
            KvCommand::Insert { path, key, value } => {
                Kv::new(path)?.insert(key, value)?;
                Ok(true)
            }
            KvCommand::WithPrefix { path, prefix } => {
                let kv = Kv::new(path)?;
                let cursor = kv.with_prefix(&prefix)?;
                for row in cursor {
                    let (k, v) = row?;
                    println!("{}", serde_json::to_string(&(k, v))?);
                }
                Ok(true)
            }
        },
        Command::Log { cmd: _ } => todo!(),
    }
}
