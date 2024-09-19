use crate::{Line, Result};
use color_eyre::eyre::eyre;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat as CPALSampleFormat,
};
use tokio::sync::broadcast;
use tokio_stream::{
    wrappers::ReceiverStream,
    {Stream, StreamExt},
};

pub struct Auth {
    pub region: String,
    pub key: String,
}

// Stream is actually Send, but crate is apparently unnecessarily
// conservative on android
// https://github.com/RustAudio/cpal/issues/818
struct StreamSendWrapper(cpal::Stream);
unsafe impl Send for StreamSendWrapper {}

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
    let (stream, microphone) = listen_from_default_input().await?;
    let microphone = StreamSendWrapper(microphone);

    // Start the microphone.
    microphone.0.play()?;

    let mut events = client
        .recognize(
            stream,
            azure_speech::recognizer::ContentType::Wav,
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

// This utility function creates a stream from the default input device.
// The audio headers are sent first, then the audio data.
async fn listen_from_default_input(
) -> Result<(impl Stream<Item = Vec<u8>>, cpal::Stream)> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| eyre!("Failed to get default input device"))?;
    let device_config = device.default_input_config()?;

    let config = device_config.clone().into();

    let (tx, rx) = tokio::sync::mpsc::channel(1024);

    tx.send(
        hound::WavSpec {
            sample_rate: device_config.sample_rate().0,
            channels: device_config.channels(),
            bits_per_sample: (device_config.sample_format().sample_size() * 8)
                as u16,
            sample_format: match device_config.sample_format().is_float() {
                true => hound::SampleFormat::Float,
                false => hound::SampleFormat::Int,
            },
        }
        .into_header_for_infinite_file(),
    )
    .await?;

    let err = |err| tracing::error!("Trying to stream input: {err}");

    let stream = match device_config.sample_format() {
        CPALSampleFormat::I8 => device.build_input_stream(
            &config,
            move |data: &[i8], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::U8 => device.build_input_stream(
            &config,
            move |data: &[u8], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::I32 => device.build_input_stream(
            &config,
            move |data: &[i32], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::U32 => device.build_input_stream(
            &config,
            move |data: &[u32], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::I64 => device.build_input_stream(
            &config,
            move |data: &[i64], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::U64 => device.build_input_stream(
            &config,
            move |data: &[u64], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        CPALSampleFormat::F64 => device.build_input_stream(
            &config,
            move |data: &[f64], _| {
                data.iter().for_each(|d| {
                    tx.try_send(d.to_le_bytes().to_vec()).unwrap_or(())
                })
            },
            err,
            None,
        ),
        _ => panic!("Unsupported sample format"),
    }?;

    Ok((ReceiverStream::new(rx), stream))
}
