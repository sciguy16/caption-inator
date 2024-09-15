use crate::{Line, Result};
use std::{path::Path, str::FromStr, time::Duration};
use tokio::sync::broadcast;

const LINE_DELAY: Duration = Duration::from_millis(300);

pub fn run(tx: broadcast::Sender<Line>, file: &Path) -> Result<()> {
    let content = std::fs::read_to_string(file)?;
    let lines = content
        .lines()
        .filter(|line| !line.is_empty())
        .map(Line::from_str)
        .collect::<Result<Vec<_>>>()?;
    tokio::task::spawn(run_inner(tx, lines));
    Ok(())
}

async fn run_inner(tx: broadcast::Sender<Line>, lines: Vec<Line>) {
    let mut interval = tokio::time::interval(LINE_DELAY);
    let mut lines_iter = lines.iter().cycle();
    loop {
        tx.send(lines_iter.next().unwrap().clone()).unwrap();
        interval.tick().await;
    }
}
