# videotrim

A command-line tool that removes filler words, long silences, false starts, and off-topic rambling from a talking-head video — automatically, and entirely on your own machine.

You give it a video and a sentence describing what you want gone. It gives you back a cut version, plus every clip it removed so you can check its work.

Nothing leaves your machine. No API keys, no upload, no per-minute billing.

## How it works

The pipeline is four stages, all local:

1. **Extract audio** (`src/audio.rs`) — ffmpeg pulls the audio track out of your video and converts it to 16 kHz mono PCM, which is the format Whisper expects.
2. **Transcribe** (`src/transcibe.rs`) — Whisper (via `whisper-rs`) turns the audio into a list of timestamped segments. The model is downloaded from Hugging Face on first use and cached, so later runs skip the download.
3. **Plan the edit** (`src/llm.rs`) — the transcript and your instructions go to a local Ollama model, which returns a structured list of time ranges to cut. Each cut carries the transcript text it covers and a short reason, and the plan as a whole carries the model's overall `reasoning`. Structured JSON output is enforced from the `EditPlan` schema, so the model can't hand back free-form prose. If the model decides nothing is worth cutting, the run stops and prints its reasoning rather than handing you back a copy of your own video.
4. **Cut and stitch** (`src/editor.rs`) — the cut list is inverted into the ranges worth keeping, each range is sliced out with ffmpeg, and the keepers are concatenated back into one video. Slicing uses stream copy (`-c copy`), so there's no re-encode and the process is fast.

Progress through the stages is shown with a spinner (`src/spinner.rs`).

## Requirements

Four things, on every platform:

| | Why |
| --- | --- |
| **Rust** 1.85+ | The crate is edition 2024. |
| **ffmpeg** on `PATH` | Does all audio extraction and video cutting. |
| **C/C++ toolchain, CMake, libclang** | `whisper-rs` builds whisper.cpp from source and runs bindgen. Needed at *build* time, not just to run. |
| **Ollama**, running, with a model pulled | Plans the edit. Reached at its default `http://localhost:11434`. |

Install instructions per platform below. Rust itself is the same everywhere except for the installer:

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

```powershell
# Windows
winget install Rustlang.Rustup
```

### Linux (Debian / Ubuntu)

```bash
sudo apt update
sudo apt install ffmpeg build-essential cmake libclang-dev pkg-config

curl -fsSL https://ollama.com/install.sh | sh
ollama serve            # leave running in its own terminal
ollama pull qwen2.5:7b  # or any model you prefer
```

On Fedora/RHEL use `sudo dnf install ffmpeg gcc gcc-c++ cmake clang-devel pkgconf`; on Arch, `sudo pacman -S ffmpeg base-devel cmake clang`. (ffmpeg on Fedora needs RPM Fusion enabled.)

### macOS

```bash
xcode-select --install       # C/C++ toolchain
brew install ffmpeg cmake llvm
brew install ollama
ollama serve
ollama pull qwen2.5:7b
```

Apple Silicon builds whisper.cpp with Metal support available, though it isn't switched on here — see the GPU note under limitations. If bindgen can't find libclang, point it at Homebrew's copy:

```bash
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

### Windows (native)

Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with the **Desktop development with C++** workload — that provides the MSVC compiler and CMake. Then:

```powershell
winget install Gyan.FFmpeg
winget install LLVM.LLVM
winget install Ollama.Ollama
```

Bindgen needs to find libclang; if the build fails looking for it, set:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

Then start Ollama (it runs as a background service after install) and pull a model:

```powershell
ollama pull qwen2.5:7b
```

Open a fresh terminal after installing so `PATH` changes take effect, and verify with `ffmpeg -version`.

### Windows (WSL2 — recommended)

The native path works, but WSL2 is less trouble: you get the Linux toolchain and avoid MSVC/bindgen friction.

```bash
wsl --install -d Ubuntu    # from PowerShell, if you don't have it yet
```

Then follow the **Linux** instructions inside the WSL shell. Two things worth knowing:

- Keep the repo on the Linux filesystem (`~/rust_projects/...`), not under `/mnt/c/`. Building across the 9p mount is dramatically slower.
- If you'd rather run Ollama natively on Windows and reach it from WSL, you'll need to point it at the Windows host — this project uses `Ollama::default()`, which assumes `localhost`, so the simplest setup is installing Ollama *inside* WSL too.

## Build

```bash
cargo build --release
```

The first build is slow — it's compiling whisper.cpp. Later builds are quick.

## Usage

```bash
cargo run --release -- \
  --input talk.mp4 \
  --output ./out \
  --user_instructions "Remove filler words and any tangent about the weather" \
  --whisper_model small.en \
  --ollama_model qwen2.5:7b \
  --show_reasons
```

On Windows PowerShell, use backticks instead of backslashes for line continuation, or just put it on one line.

| Flag | Meaning |
| --- | --- |
| `--input` / `-i` | Path to the source video. Any format ffmpeg can read. |
| `--output` / `-o` | Directory for all output. Created if missing. |
| `--user_instructions` | Free-text guidance passed to the model alongside the transcript. |
| `--whisper_model` | Whisper model name, e.g. `tiny.en`, `base.en`, `small.en`, `medium.en`. |
| `--ollama_model` | Any model you've pulled in Ollama, e.g. `qwen2.5:7b`. |
| `--show_reasons` / `-r` | Print each removed clip and the model's reason for cutting it. Off by default. |

Everything except `--show_reasons` is required.

Whisper model names map to the `ggml-<name>.bin` files in the [whisper.cpp Hugging Face repo](https://huggingface.co/ggerganov/whisper.cpp). They're downloaded on first use and cached in `videotrim_rust_cli_whisper_model/` under your OS cache directory (`~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows). Larger models transcribe more accurately and more slowly; `small.en` is a reasonable starting point for English.

### What you get

Inside your `--output` directory:

```
out/
├── edited_video.mp4    the finished cut
├── audio.wav           extracted audio (intermediate)
├── transcript.json     timestamped transcript, useful for debugging
├── used_clips/         every kept segment, plus the ffmpeg concat list
└── removed_clips/      every segment the model decided to cut
```

`removed_clips/` exists so you can audit the edit. If the result cut something it shouldn't have, the evidence is right there, and `transcript.json` shows what the model was reading when it made the call.

## Notes and current limitations

- **English only.** The transcription language is hardcoded to `en` in `src/transcibe.rs`.
- **GPU is requested but not enabled.** The code calls `use_gpu(true)`, but no GPU backend feature (`cuda`, `vulkan`, `metal`) is enabled on `whisper-rs` in `Cargo.toml`, so transcription runs on CPU. Enabling one is the single biggest speedup available for long videos.
- **Stream-copy cuts snap to keyframes.** Because clips are extracted with `-c copy`, cut points land on the nearest keyframe rather than the exact timestamp. This is what makes it fast; it also means cuts can be off by a fraction of a second. Re-encoding instead would be frame-accurate and much slower.
- **Errors panic.** Failures before the final stage abort with a message rather than degrading gracefully. `src/error.rs` is a placeholder for a real error type.
- **A failed final render still exits 0.** `process_video` errors are printed, not propagated, so scripts can't detect them from the exit code.
- **Edit quality tracks model quality.** A 7B model does a decent job on filler words and obvious tangents. Nuanced instructions benefit from a larger one.
