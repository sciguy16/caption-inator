use crate::{ControlMessage, Language, Line, Result, RunState};
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

const TEST_LINES: &str = include_str!("test-data.txt");

pub struct Auth {
    pub region: String,
    pub key: String,
}

struct SetupState {
    language: String,
    wordlist: Option<String>,
}

impl Default for SetupState {
    fn default() -> Self {
        Self {
            language: crate::LANGUAGE_OPTIONS[0].into(),
            wordlist: None,
        }
    }
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
    let mut setup_state = SetupState::default();

    let azure_auth =
        azure_speech::Auth::from_subscription(auth.region, auth.key);

    loop {
        run_state = match run_state {
            RunState::Stopped => {
                wait_for_transition(&mut control_rx, &mut setup_state).await
            }
            RunState::Running => {
                match do_run(
                    &tx,
                    &mut control_rx,
                    &mut setup_state,
                    &azure_auth,
                )
                .await
                {
                    Ok(state) => state,
                    Err(err) => {
                        error!("{err}");
                        RunState::Stopped
                    }
                }
            }
            RunState::Test => {
                run_test(&tx, &mut control_rx, &mut setup_state).await
            }
        };
    }
}

async fn wait_for_transition(
    control_rx: &mut mpsc::Receiver<ControlMessage>,
    setup_state: &mut SetupState,
) -> RunState {
    loop {
        match control_rx.recv().await.unwrap() {
            ControlMessage::SetState(new_state) => break new_state,
            ControlMessage::GetState(reply) => {
                let _ = reply.send(RunState::Stopped);
            }
            other => handle_lang_and_wordlist(other, setup_state),
        }
    }
}

fn langauge_from_language(lang: &str) -> azure_speech::recognizer::Language {
    match lang {
        "en-GB" => azure_speech::recognizer::Language::EnGb,
        "en-IE" => azure_speech::recognizer::Language::EnIe,
        "en-US" => azure_speech::recognizer::Language::EnUs,
        "ja-JP" => azure_speech::recognizer::Language::JaJp,
        _ => azure_speech::recognizer::Language::EnGb,
    }
}

async fn do_run(
    tx: &broadcast::Sender<Line>,
    control_rx: &mut mpsc::Receiver<ControlMessage>,
    setup_state: &mut SetupState,
    auth: &azure_speech::Auth,
) -> Result<RunState> {
    let azure_config = azure_speech::recognizer::Config::default()
        .set_language(langauge_from_language(&setup_state.language));
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
                            info!("Shutting down azure speech client");
                            if let Err(err) = client.disconnect().await{
                                error!("{err:?}");
                            }
                            return Ok(new_state);
                        }
                    }
                    ControlMessage::GetState(reply) => {
                        let _ = reply.send(RunState::Running);
                    }
            other => handle_lang_and_wordlist(other, setup_state),
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
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();

    tokio::task::spawn(async move {
        child.wait().await.unwrap();
    });

    tokio::task::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buf = [0; 1024];
        let mut errors = 0_usize;
        loop {
            match reader.read_exact(&mut buf).await {
                Ok(_) => errors = 0,
                Err(err) => {
                    warn!("{err}");
                    errors += 1;
                    if errors > 5 {
                        error!(
                            "Max errors reached, unable to read ffmpeg stream"
                        );
                        break;
                    }
                    continue;
                }
            }
            if tx.send(buf.to_vec()).await.is_err() {
                info!("Stream closed");
                break;
            }
        }
    });

    Ok(ReceiverStream::new(rx))
}

async fn run_test(
    tx: &broadcast::Sender<Line>,
    control_rx: &mut mpsc::Receiver<ControlMessage>,
    setup_state: &mut SetupState,
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
            other => handle_lang_and_wordlist(other, setup_state),
                }

            }
        }
    }
}

fn handle_lang_and_wordlist(msg: ControlMessage, setup_state: &mut SetupState) {
    match msg {
        ControlMessage::GetLanguage(reply) => {
            let _ = reply.send(Language {
                options: crate::LANGUAGE_OPTIONS
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
                current: setup_state.language.clone(),
            });
        }
        ControlMessage::SetLanguage(choice) => {
            if crate::LANGUAGE_OPTIONS.contains(&choice.as_str()) {
                setup_state.language = choice;
            } else {
                warn!("Invalid language choice `{choice}`");
            }
        }
        other => panic!("Unreachable: {other:?}"),
    }
}
