use crate::{Line, Result};
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::broadcast,
};

// spx recognize --microphone --phrases @/tmp/words.txt --language en-GB

pub fn start(tx: broadcast::Sender<Line>) {
    tokio::task::spawn(async move { start_inner(tx).await.unwrap() });
}

async fn start_inner(tx: broadcast::Sender<Line>) -> Result<()> {
    let mut child = tokio::process::Command::new("spx")
        .args(["recognize", "--microphone", "--language", "en-GB"])
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();

    tokio::task::spawn(async move {
        let status = child
            .wait()
            .await
            .expect("child process encountered an error");

        println!("child status was: {}", status);
    });

    let mut reader = BufReader::new(stdout).lines();
    while let Some(line) = reader.next_line().await? {
        if let Ok(line) = line.parse() {
            println!("Line: {line:?}");
            tx.send(line)?;
        }
    }

    Ok(())
}
