use clap::Parser;
use color_eyre::{eyre::eyre, Result};
use serde::Serialize;
use std::{path::PathBuf, str::FromStr};
use tokio::sync::broadcast;

#[macro_use]
extern crate tracing;

mod listener;
mod replay;
mod server;

const PREFIX_RECOGNISING: &str = "RECOGNIZING: ";
const PREFIX_RECOGNISED: &str = "RECOGNIZED: ";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
enum Line {
    Recognising(String),
    Recognised(String),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
enum RunState {
    Stopped,
    Running,
    Test,
}

impl FromStr for Line {
    type Err = color_eyre::Report;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(line) = s.strip_prefix(PREFIX_RECOGNISING) {
            Ok(Self::Recognising(line.into()))
        } else if let Some(line) = s.strip_prefix(PREFIX_RECOGNISED) {
            Ok(Self::Recognised(line.into()))
        } else {
            Err(eyre!("Invalid input"))
        }
    }
}

#[derive(Parser)]
struct Args {
    #[clap(long, help = "Replay a text file over the websocket")]
    replay: Option<PathBuf>,
    #[clap(long, help = "Directory to serve frontend assets out of")]
    frontend: Option<PathBuf>,
    #[clap(long, help = "Azure speech services region")]
    region: Option<String>,
    #[clap(long, help = "Azure speech services key")]
    key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    init_tracing();

    let args = Args::parse();

    let (tx, _rx) = broadcast::channel(10);

    if let Some(file) = args.replay {
        info!("Starting in replay mode from `{}`", file.display());
        replay::run(tx.clone(), &file)?;
    } else {
        info!("Starting in listener mode");
        let auth = match (args.region, args.key) {
            (Some(region), Some(key)) => listener::Auth { region, key },
            _ => Err(eyre!("Region and key are required for Azure listener"))?,
        };
        listener::start(tx.clone(), auth);
    }

    server::run(tx, args.frontend).await?;

    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{filter::LevelFilter, EnvFilter};

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::DEBUG.into())
                .from_env_lossy(),
        )
        .with_line_number(true)
        .init();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn line_from_str() {
        let cases = [
            ("RECOGNIZING: game", Line::Recognising("game".into())),
            (
                "RECOGNIZING: let's get into business",
                Line::Recognising("let's get into business".into()),
            ),
            (
                "RECOGNIZED: You can't do it again. Just so you know. \
                Marlene's still asleep.",
                Line::Recognised(
                    "You can't do it again. Just so you know. Marlene's \
                    still asleep."
                        .into(),
                ),
            ),
        ];

        for (input, parsed) in cases.into_iter() {
            println!("case: `{input}`");
            assert_eq!(parsed, input.parse().unwrap());
        }

        let _ = "you just lost the game".parse::<Line>().unwrap_err();
    }
}
