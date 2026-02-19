# CLAUDE.md

This file provides guidance to Claude Code when working with code in this project.

## Project Overview

Pipeline to transcribe Zoom meeting recordings into per-JIRA-ticket markdown files with speaker attribution. Works with local recordings from [zoom-recorder](../zoom-recorder/).

## Pipeline Flow

```
zoom-recorder output    →  whisper-wrapper.py  →  diarize.py  →  segment-transcript.py  →  fetch-jira-info.sh
(audio.wav + timestamps)   (transcribe)           (speakers)     (split by ticket)          (JIRA metadata)
```

Orchestrated by `process-local-recordings.sh` (steps 1-3) and `process-recordings.sh` (all steps).

## Running

```bash
# Process today's recordings (transcribe + diarize + segment + JIRA enrich)
./process-recordings.sh

# Process a specific date
./process-recordings.sh 2026-02-19

# Just transcribe + diarize + segment (no JIRA)
./process-local-recordings.sh 2026-02-19

# Run tests
~/voice-to-text-env/bin/python -m pytest tests/ -v
```

## Key Scripts

| Script | Purpose |
|--------|---------|
| `process-recordings.sh` | Master orchestrator — runs all steps |
| `process-local-recordings.sh` | Transcribe → diarize → segment by ticket |
| `whisper-wrapper.py` | Whisper transcription, outputs JSON with confidence scores |
| `diarize.py` | pyannote speaker diarization + hallucination filtering + segment merging |
| `segment-transcript.py` | Splits segments by JIRA ticket marks, formats speaker labels |
| `fetch-jira-info.sh` | Appends JIRA metadata to transcript .md files |

### Legacy (unused)

`download-recordings.sh`, `transcribe-all.sh` — from the original S3-based pipeline.

## Environment

- Python virtualenv: `~/voice-to-text-env/bin/python` (whisper, pyannote.audio, torch with CUDA)
- Config: `.env` (copy from `.env.example`)
- Recordings source: `~/zoom-recordings/` (zoom-recorder output)
- Output: `recordings/` directory (per-ticket .md files)

## Architecture Notes

### Speaker Diarization (`diarize.py`)

Three processing stages run in order:
1. **Filter hallucinations** — removes segments with high `no_speech_prob`, low `avg_logprob`, or non-Latin characters
2. **Merge speakers** — assigns each whisper segment a speaker label by maximum time overlap with pyannote turns. Unknown segments inherit the nearest speaker.
3. **Merge consecutive** — combines adjacent same-speaker segments into single blocks

pyannote.audio 4.x returns `DiarizeOutput`, not `Annotation` directly — use `.speaker_diarization` to get the `Annotation` object. The `Pipeline.from_pretrained()` parameter is `token=`, not `use_auth_token=`.

Diarization failure is non-fatal: the pipeline falls back to unlabeled transcripts.

### Transcript Segmentation (`segment-transcript.py`)

- When segments have speaker labels, text is formatted as `**Speaker N:** text` and joined with `\n\n`
- Without speakers, text is space-joined (backward compatible)
- JIRA ticket boundaries come from `timestamps.json` marks created by zoom-recorder

### Output Format

```markdown
# GT-1234 - Meeting Transcript

**Source:** 2026-02-19-093015/audio.wav
**Date:** 2026-02-19
**Segment:** 00:00:00 - 00:08:42

---

**Speaker 1:** So this one is about the freemium BYO experience.

**Speaker 2:** Yeah, we want to A/B test three variations.
```

## Testing

Tests use `~/voice-to-text-env/bin/python` (needs pyannote.core for diarize tests). Diarize tests mock the pyannote Pipeline using real `Annotation` objects — no model download needed.

```bash
~/voice-to-text-env/bin/python -m pytest tests/ -v
```
