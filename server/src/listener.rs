use crate::{ControlMessage, Line, Result, RunState};
use color_eyre::eyre::eyre;
use std::{process::Stdio, str::FromStr, time::Duration};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::{broadcast, mpsc},
};
use tokio_stream::{
    wrappers::ReceiverStream,
    {Stream, StreamExt},
};

const TEST_LINES: &str = include_str!("../../test-data/output.txt");

pub struct Auth {
    pub region: String,
    pub key: String,
}

// spx recognize --microphone --phrases @/tmp/words.txt --language en-GB

pub fn start(
    tx: broadcast::Sender<Line>,
    control_rx: mpsc::Receiver<ControlMessage>,
    auth: Auth,
) {
    tokio::task::spawn(async move {
        start_inner(tx, control_rx, auth).await.unwrap()
    });
}

// State machine:
// - Stopped: wait for control channel message to transition to other state
// - Running: start azure client and then select! on that and the control channel
// - Test: start test loop and then select! on that and the control channel
async fn start_inner(
    tx: broadcast::Sender<Line>,
    mut control_rx: mpsc::Receiver<ControlMessage>,
    auth: Auth,
) -> Result<()> {
    let mut run_state = RunState::Stopped;

    let azure_auth =
        azure_speech::Auth::from_subscription(auth.region, auth.key);

    loop {
        run_state = match run_state {
            RunState::Stopped => wait_for_transition(&mut control_rx).await,
            RunState::Running => {
                match do_run(&tx, &mut control_rx, &azure_auth).await {
                    Ok(state) => state,
                    Err(err) => {
                        error!("{err}");
                        RunState::Stopped
                    }
                }
            }
            RunState::Test => run_test(&tx, &mut control_rx).await,
        };
    }
}

async fn wait_for_transition(
    control_rx: &mut mpsc::Receiver<ControlMessage>,
) -> RunState {
    loop {
        match control_rx.recv().await.unwrap() {
            ControlMessage::SetState(new_state) => break new_state,
            ControlMessage::GetState(reply) => {
                let _ = reply.send(RunState::Stopped);
            }
        }
    }
}

async fn do_run(
    tx: &broadcast::Sender<Line>,
    control_rx: &mut mpsc::Receiver<ControlMessage>,
    auth: &azure_speech::Auth,
) -> Result<RunState> {
    let azure_config = azure_speech::recognizer::Config::default();
    let client =
        azure_speech::recognizer::Client::connect(auth.clone(), azure_config)
            .await
            .map_err(|err| eyre!("{err:?}"))?;

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

    loop {
        tokio::select! {
            event = events.next() => {
                let Some(event) = event else { break; };
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
            msg = control_rx.recv() => {
                let msg = msg.unwrap();
                match msg {
                    ControlMessage::SetState(new_state) => {
                        if new_state != RunState::Running {
                            return Ok(new_state);
                        }
                    }
                    ControlMessage::GetState(reply) => {
                        let _ = reply.send(RunState::Running);
                    }
                }

            }
        }
    }

    Ok(RunState::Stopped)
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

async fn run_test(
    tx: &broadcast::Sender<Line>,
    control_rx: &mut mpsc::Receiver<ControlMessage>,
) -> RunState {
    const LINE_DELAY: Duration = Duration::from_millis(300);

    let mut interval = tokio::time::interval(LINE_DELAY);
    let lines = TEST_LINES
        .lines()
        .filter(|line| !line.is_empty())
        .map(Line::from_str)
        .collect::<Result<Vec<_>>>()
        .unwrap();
    let mut lines_iter = lines.iter().cycle();

    loop {
        tokio::select! {
            _ = interval.tick() => {
                tx.send(lines_iter.next().unwrap().clone()).unwrap();


            }
            msg = control_rx.recv() => {
                let msg = msg.unwrap();
                match msg {
                    ControlMessage::SetState(new_state) => {
                        if new_state != RunState::Test {
                            return new_state;
                        }
                    }
                    ControlMessage::GetState(reply) => {
                        let _ = reply.send(RunState::Test);
                    }
                }

            }
        }
    }
}
