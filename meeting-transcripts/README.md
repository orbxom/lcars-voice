# Meeting Transcripts

Automated pipeline to transcribe meeting recordings into markdown with speaker attribution. Works with local recordings from [lcars-voice](../lcars-voice/).

## Quick Start

```bash
# First time setup
cp .env.example .env
# Edit .env with your credentials

# Process today's recordings
./process-recordings.sh

# Process a specific date
./process-recordings.sh 2026-02-19
```

## Setup

### Prerequisites

- Python virtualenv with Whisper and pyannote.audio at `~/voice-to-text-env` (or set `PYTHON_ENV` in `.env`)
- FFmpeg
- HuggingFace token with access to [pyannote/speaker-diarization-3.1](https://huggingface.co/pyannote/speaker-diarization-3.1) (for speaker diarization)
- [lcars-voice](../lcars-voice/) producing recordings in `~/.local/share/lcars-voice/recordings/`

### Configuration

Copy `.env.example` to `.env` and fill in:

```bash
# Whisper
WHISPER_MODEL=large-v3

# Speaker diarization
HF_TOKEN=your-huggingface-token

# Paths (optional, these are the defaults)
RECORDINGS_SOURCE=~/.local/share/lcars-voice/recordings
PYTHON_ENV=~/voice-to-text-env/bin/python
```

## Workflow

1. **During meetings:** Use [lcars-voice](../lcars-voice/) in Meeting mode to record
2. **End of day:** Run the pipeline to transcribe

```bash
# Full pipeline (transcribe + diarize + segment)
./process-recordings.sh 2026-02-19

# Or step by step
./process-local-recordings.sh 2026-02-19       # Transcribe, diarize, write transcript
```

3. **Analysis:** Use the `analyzing-meeting-transcripts` Claude skill on individual `.md` files to generate deep technical analysis reports

## How It Works

The pipeline processes lcars-voice recording directories from `~/.local/share/lcars-voice/recordings/`:

```
~/.local/share/lcars-voice/recordings/
├── 2026-02-19-093015/          <- One recording session
│   ├── audio.wav               <- 16kHz mono WAV
│   └── metadata.json           <- Recording metadata
└── 2026-02-19-140022/          <- Another session
    ├── audio.wav
    └── metadata.json
```

For each recording:
1. Whisper transcribes `audio.wav` into text segments with timestamps
2. pyannote.audio runs speaker diarization, assigning each segment a speaker label (`Speaker 1`, `Speaker 2`, etc.)
3. Hallucinated segments are filtered (using whisper confidence scores and language detection)
4. Consecutive same-speaker segments are merged for readability
5. A transcript `.md` file is written (e.g., `2026-02-19-093015.md`) with speaker attribution

## Output

After processing, the `recordings/` directory contains:

- `YYYY-MM-DD-HHMMSS.md` - Transcript with speaker attribution

If the same recording session is processed again, the transcript is appended to the existing `.md` file.

## Scripts

| Script | Purpose |
|--------|---------|
| `process-recordings.sh` | Master orchestrator — runs all steps |
| `process-local-recordings.sh` | Finds recordings by date, transcribes, diarizes, writes transcript |
| `diarize.py` | Speaker diarization with pyannote + hallucination filtering |
| `segment-transcript.py` | Writes transcript markdown |
| `whisper-wrapper.py` | Python wrapper for Whisper |
| `fetch-jira-info.sh` | Standalone tool: appends JIRA metadata to transcript .md files (optional) |

### Legacy Scripts

These scripts are from the original S3-based pipeline and are no longer used:

| Script | Purpose |
|--------|---------|
| `download-recordings.sh` | Downloaded recordings from S3 |
| `transcribe-all.sh` | Transcribed using RecordingIndex.md mapping |
