# Meeting Transcripts

Automated pipeline to download meeting recordings from S3, transcribe them with Whisper, and enrich with JIRA information.

## Quick Start

```bash
# First time setup
cp .env.example .env
# Edit .env with your credentials

# Process recordings for a date
./process-recordings.sh 01-29-2026
```

## Setup

### Prerequisites

- AWS CLI configured with SSO access
- Python virtualenv with Whisper at `~/voice-to-text-env`
- JIRA API token

### Configuration

Copy `.env.example` to `.env` and fill in:

```bash
# JIRA
JIRA_URL=https://yourcompany.atlassian.net
JIRA_USER=your.email@company.com
JIRA_TOKEN=your-api-token

# AWS
AWS_PROFILE=sandbox
S3_BUCKET=growth-recordings

# Whisper (optional, defaults to large-v3)
WHISPER_MODEL=large-v3
```

## Usage

### Full Pipeline

```bash
# Simple - uses date folder and defaults from .env
./process-recordings.sh 01-29-2026

# Custom output folder
./process-recordings.sh 01-29-2026 ./output

# Full control
./process-recordings.sh s3://bucket/path ./output aws-profile
```

### Individual Scripts

```bash
# Download only
./download-recordings.sh s3://bucket/01-29-2026 ./recordings sandbox

# Transcribe only (requires RecordingIndex.md in folder)
./transcribe-all.sh ./recordings

# Fetch JIRA only (requires transcripts already created)
./fetch-jira-info.sh ./recordings
```

## S3 Folder Structure

Each date folder in S3 should contain:

```
01-29-2026/
├── RecordingIndex.md      # Maps recordings to JIRA IDs
├── 2026-01-29 10-20-14.mp4
├── 2026-01-29 10-29-58.mp4
└── ...
```

### RecordingIndex.md Format

One JIRA ID per line, in timestamp order. Multiple JIRAs for same recording joined with dashes:

```markdown
# Recording Index
GT-9516
GT-9438
GT-9544
GT-9523-GT-9524
GT-9528
GT-9525
GT-9522-GT-9556-GT-9555
```

## Output

After processing, each recording becomes:

- `GT-XXXX.mp4` - Renamed video file
- `GT-XXXX.md` - Transcript + JIRA information

JIRA attachments are downloaded to `recordings/attachments/<jira-id>/`.

## Scripts

| Script | Purpose |
|--------|---------|
| `process-recordings.sh` | Master orchestrator - runs all steps |
| `download-recordings.sh` | Downloads from S3 (idempotent) |
| `transcribe-all.sh` | Whisper transcription + rename |
| `fetch-jira-info.sh` | JIRA API enrichment |
| `whisper-wrapper.py` | Python wrapper for Whisper |
