use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::{env, time::Duration};

pub const VOICE_DISCLOSURE_VERSION: i32 = 1;
pub const VOICE_PROVIDER: &str = "openai";
pub const VOICE_MODEL: &str = "gpt-transcribe";

#[derive(Debug)]
pub enum TranscriptionError {
    Unavailable,
    Failed,
}

#[async_trait]
pub trait VoiceTranscriber: Send + Sync {
    fn available(&self) -> bool;
    async fn transcribe(&self, audio: Vec<u8>) -> Result<String, TranscriptionError>;
}

pub struct OpenAiTranscriber {
    key: Option<String>,
    audited: bool,
    client: reqwest::Client,
}

impl OpenAiTranscriber {
    pub fn from_env() -> Self {
        Self {
            key: env::var("OPENAI_API_KEY")
                .ok()
                .filter(|value| !value.is_empty()),
            audited: env::var("OPENAI_TRANSCRIPTION_AUDITED").as_deref() == Ok("true"),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(75))
                .build()
                .expect("HTTP client should initialize"),
        }
    }
}

#[derive(Deserialize)]
struct TranscriptResponse {
    text: String,
}

#[async_trait]
impl VoiceTranscriber for OpenAiTranscriber {
    fn available(&self) -> bool {
        self.key.is_some() && self.audited
    }

    async fn transcribe(&self, audio: Vec<u8>) -> Result<String, TranscriptionError> {
        if !self.available() {
            return Err(TranscriptionError::Unavailable);
        }
        let key = self.key.as_ref().ok_or(TranscriptionError::Unavailable)?;
        let file = Part::bytes(audio)
            .file_name("recording.m4a")
            .mime_str("audio/mp4")
            .map_err(|_| TranscriptionError::Failed)?;
        let response = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(key)
            .multipart(Form::new().text("model", VOICE_MODEL).part("file", file))
            .send()
            .await
            .map_err(|_| TranscriptionError::Failed)?;
        if !response.status().is_success() {
            return Err(TranscriptionError::Failed);
        }
        let result: TranscriptResponse = response
            .json()
            .await
            .map_err(|_| TranscriptionError::Failed)?;
        Ok(result.text)
    }
}
