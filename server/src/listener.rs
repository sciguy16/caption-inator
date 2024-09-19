use crate::{Line, Result};
use color_eyre::eyre::eyre;
use std::process::Stdio;
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::{broadcast, mpsc},
};
use tokio_stream::{
    wrappers::ReceiverStream,
    {Stream, StreamExt},
};

pub struct Auth {
    pub region: String,
    pub key: String,
}

// spx recognize --microphone --phrases @/tmp/words.txt --language en-GB

pub fn start(tx: broadcast::Sender<Line>, auth: Auth) {
    tokio::task::spawn(async move { start_inner(tx, auth).await.unwrap() });
}

async fn start_inner(tx: broadcast::Sender<Line>, auth: Auth) -> Result<()> {
    let auth = azure_speech::Auth::from_subscription(auth.region, auth.key);

    let config = azure_speech::recognizer::Config::default();

    let client = azure_speech::recognizer::Client::connect(auth, config)
        .await
        .map_err(|err| eyre!("{err:?}"))?;

    // Using this utility, I'm creating an audio stream from the default input device.
    // The audio headers are sent first, then the audio data.
    // As the audio is raw, the WAV format is used.
    let stream = listen_from_default_input().await?;

    let mut events = client
        .recognize(
            stream,
            azure_speech::recognizer::ContentType::Webm,
            azure_speech::recognizer::Details::stream("mac", "stream"),
        )
        .await
        .map_err(|err| eyre!("{err:?}"))?;

    tracing::info!("... Starting to listen from microphone ...");

    while let Some(event) = events.next().await {
        dbg!(&event);
        use azure_speech::recognizer::Event;
        match event {
            Ok(Event::Recognized(_, result, _, _, _)) => {
                tx.send(Line::Recognised(result.text.clone()))?;
            }
            Ok(Event::Recognizing(_, result, _, _, _)) => {
                tx.send(Line::Recognising(result.text.clone()))?;
            }
            Err(err) => {
                error!("{err:?}");
            }
            _ => {}
        }
    }

    tracing::info!("Completed!");

    Ok(())
}

// ffmpeg -y -f pulse -ac 2 -i default -f webm /dev/stdout
async fn listen_from_default_input() -> Result<impl Stream<Item = Vec<u8>>> {
    let (tx, rx) = mpsc::channel(10);

    let mut child = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "pulse",
            "-ac",
            "2",
            "-i",
            "default",
            "-f",
            "webm",
            "/dev/stdout",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();

    tokio::task::spawn(async move {
        child.wait().await.unwrap();
    });

    tokio::task::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf = [0; 1024];
        loop {
            reader.read_exact(&mut buf).await.unwrap();
            tx.send(buf.to_vec()).await.unwrap();
        }
    });

    Ok(ReceiverStream::new(rx))
}
