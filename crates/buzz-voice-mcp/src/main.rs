use anyhow::Result;
use buzz_voice::pocket::{load_text_to_speech, load_voice_style};
use rodio::{buffer::SamplesBuffer, OutputStream, Sink};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() -> Result<()> {
    let home = std::env::var("HOME").unwrap();
    let model_dir = format!("{}/.buzz/models/pocket-tts", home);

    // Load ONNX engine and voice style
    let engine = load_text_to_speech(&model_dir).expect("Failed to load April PocketTts");
    // voice style loads the specific style numpy array/file from the bundle. Assuming "charles.wav" style?
    // wait, pocket_voice.rs usually has load_voice_style which takes a path.
    let style_path = format!("{}/charles.wav", model_dir);
    let style =
        load_voice_style(Path::new(&style_path)).expect("Failed to load charles voice style");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req: Value = serde_json::from_str(&line)?;
        let id = req.get("id");
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "initialize" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {},
                        "serverInfo": { "name": "buzz-voice-mcp", "version": "1.0.0" }
                    }
                });
                writeln!(stdout, "{}", resp)?;
            }
            "tools/list" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [{
                            "name": "stream_voice",
                            "description": "Speak text aloud using local Buzz ONNX TTS",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "text": { "type": "string" }
                                },
                                "required": ["text"]
                            }
                        }]
                    }
                });
                writeln!(stdout, "{}", resp)?;
            }
            "tools/call" => {
                if let Some(text) = req
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .and_then(|a| a.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if let Ok(samples) = engine.synth_chunk(text, "en", &style, 20) {
                        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
                            if let Ok(sink) = Sink::try_new(&stream_handle) {
                                let buffer = SamplesBuffer::new(1, 24000, samples);
                                sink.append(buffer);
                                sink.sleep_until_end();
                            }
                        }
                    }
                }

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": "Voice played successfully" }]
                    }
                });
                writeln!(stdout, "{}", resp)?;
            }
            _ => {
                if id.is_some() {
                    let resp = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } });
                    writeln!(stdout, "{}", resp)?;
                }
            }
        }
        stdout.flush()?;
    }
    Ok(())
}
