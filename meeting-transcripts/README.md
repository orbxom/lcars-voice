# Meeting Transcripts

Automated pipeline to transcribe Zoom meeting recordings and enrich with JIRA information. Works with local recordings from the [zoom-recorder](../zoom-recorder/) tool.

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

- Python virtualenv with Whisper at `~/voice-to-text-env` (or set `PYTHON_ENV` in `.env`)
- FFmpeg
- JIRA API token
- [zoom-recorder](../zoom-recorder/) producing recordings in `~/zoom-recordings/`

### Configuration

Copy `.env.example` to `.env` and fill in:

```bash
# JIRA
JIRA_URL=https://yourcompany.atlassian.net
JIRA_USER=your.email@company.com
JIRA_TOKEN=your-api-token

# Whisper
WHISPER_MODEL=large-v3

# Paths (optional, these are the defaults)
RECORDINGS_SOURCE=~/zoom-recordings
PYTHON_ENV=~/voice-to-text-env/bin/python
```

## Workflow

1. **During meetings:** Use [zoom-recorder](../zoom-recorder/) to record Zoom calls, marking JIRA tickets as topics come up
2. **End of day:** Run the pipeline to transcribe and enrich

```bash
# Full pipeline (transcribe + segment + JIRA enrich)
./process-recordings.sh 2026-02-19

# Or step by step
./process-local-recordings.sh 2026-02-19       # Transcribe and segment by ticket
./fetch-jira-info.sh ./recordings               # Enrich with JIRA metadata
```

3. **Analysis:** Use the `analyzing-meeting-transcripts` Claude skill on individual `.md` files to generate deep technical analysis reports

## How It Works

The pipeline processes zoom-recorder output directories from `~/zoom-recordings/`:

```
~/zoom-recordings/
├── 2026-02-19-093015/          <- One recording session
│   ├── audio.wav               <- 16kHz mono WAV
│   ├── timestamps.json         <- JIRA ticket marks with time positions
│   └── metadata.json           <- Recording metadata
└── 2026-02-19-140022/          <- Another session
    ├── audio.wav
    ├── timestamps.json
    └── metadata.json
```

For each recording:
1. Whisper transcribes `audio.wav` into text segments with timestamps
2. Segments are matched to JIRA tickets using `timestamps.json` marks
3. Per-ticket `.md` files are written (e.g., `GT-9516.md`)
4. JIRA metadata (summary, status, subtasks, comments, attachments) is appended

## Output

After processing, the `recordings/` directory contains:

- `GT-XXXX.md` - Transcript segment + JIRA information
- `attachments/<jira-id>/` - Downloaded JIRA image attachments

If the same ticket is discussed in multiple recording sessions, the transcript segments are appended to the same `.md` file.

## Scripts

| Script | Purpose |
|--------|---------|
| `process-recordings.sh` | Master orchestrator - runs all steps |
| `process-local-recordings.sh` | Finds recordings by date, transcribes, segments by ticket |
| `fetch-jira-info.sh` | JIRA API enrichment |
| `segment-transcript.py` | Splits whisper output by JIRA timestamp marks |
| `whisper-wrapper.py` | Python wrapper for Whisper |

### Legacy Scripts

These scripts are from the original S3-based pipeline and are no longer used:

| Script | Purpose |
|--------|---------|
| `download-recordings.sh` | Downloaded recordings from S3 |
| `transcribe-all.sh` | Transcribed using RecordingIndex.md mapping |
