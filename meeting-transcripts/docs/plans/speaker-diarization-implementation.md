# Speaker Diarization Implementation Plan

> **For Claude:** Implement this plan using an agent team. Dispatch parallel agents where tasks are independent. Run tests along the way. Perform a code review when implementation is complete, then run a manual end-to-end test using an existing recording file.

## Goal

Add speaker diarization to the meeting transcripts pipeline so that transcripts attribute speech to distinct speakers (Speaker 1, Speaker 2, etc.). Speakers don't need to be identified by name.

## Architecture

Speaker diarization runs at the transcription layer. The approach:

1. **Whisper** transcribes audio into text segments with timestamps (already working)
2. **pyannote.audio** runs diarization on the same audio, producing speaker time ranges
3. **Merge step** assigns each whisper segment a speaker label by matching time ranges
4. **Downstream** — `segment-transcript.py` preserves speaker labels in `.md` output

## Environment

- Python virtualenv: `~/voice-to-text-env/bin/python`
- PyTorch: `2.9.1+cu128` with CUDA (RTX 3080 Ti)
- pyannote.audio: **not yet installed** (needs `pip install pyannote.audio`)
- pyannote models are gated on HuggingFace — requires accepting the license and a HuggingFace token
- Project root: `/home/zknowles/personal/claude-tools/meeting-transcripts`
- Git repo root: `/home/zknowles/personal/claude-tools`

## Important: HuggingFace Token Setup

Before implementing, you need to handle the HuggingFace token for pyannote model access:

1. Check if a HuggingFace token is already configured: `$HOME/voice-to-text-env/bin/python -c "from huggingface_hub import HfFolder; print(HfFolder.get_token())"`
2. If not, ask the user to:
   - Accept the pyannote model license at https://huggingface.co/pyannote/speaker-diarization-3.1
   - Accept the segmentation model license at https://huggingface.co/pyannote/segmentation-3.0
   - Provide their HuggingFace token
3. Add `HF_TOKEN` to `.env.example` and load it in the pipeline

---

## Task 1: Install pyannote.audio

**Steps:**
1. Install into the whisper virtualenv: `~/voice-to-text-env/bin/pip install pyannote.audio`
2. Verify: `~/voice-to-text-env/bin/python -c "from pyannote.audio import Pipeline; print('pyannote OK')"`
3. Check HuggingFace token situation (see above)

---

## Task 2: Create diarize.py + tests

**File:** `diarize.py`

A standalone Python script that:
1. Runs pyannote speaker diarization on an audio file
2. Merges speaker labels with whisper segments (from JSON)
3. Outputs enriched JSON with speaker labels added to each segment

**Interface:**
```bash
~/voice-to-text-env/bin/python diarize.py <audio-file> <whisper-json-file> [--hf-token TOKEN]
# Outputs JSON to stdout
```

**Input** (whisper JSON, already produced by whisper-wrapper.py):
```json
{
  "text": "full transcript",
  "language": "en",
  "segments": [
    {"start": 0.0, "end": 5.2, "text": " Hello everyone."},
    {"start": 5.2, "end": 12.1, "text": " Let's discuss the first ticket."}
  ]
}
```

**Output** (enriched JSON):
```json
{
  "text": "full transcript",
  "language": "en",
  "segments": [
    {"start": 0.0, "end": 5.2, "text": " Hello everyone.", "speaker": "Speaker 1"},
    {"start": 5.2, "end": 12.1, "text": " Let's discuss the first ticket.", "speaker": "Speaker 2"}
  ]
}
```

**Merging algorithm:**
- pyannote produces a diarization object with turns: `(start_time, end_time, speaker_label)`
- For each whisper segment, find the pyannote turn that overlaps the most with the segment's time range
- Assign that turn's speaker label to the segment
- pyannote uses labels like `SPEAKER_00`, `SPEAKER_01` — map these to `Speaker 1`, `Speaker 2`, etc.
- If a segment has no overlapping diarization turn, label it as `Unknown`

**Implementation sketch:**
```python
from pyannote.audio import Pipeline
import json, sys

def diarize(audio_path, hf_token=None):
    pipeline = Pipeline.from_pretrained(
        "pyannote/speaker-diarization-3.1",
        use_auth_token=hf_token
    )
    diarization = pipeline(audio_path)
    # Returns a pyannote Annotation object
    # Iterate with: for turn, _, speaker in diarization.itertracks(yield_label=True)
    return diarization

def merge_speakers(segments, diarization):
    # Build list of (start, end, speaker) turns
    turns = [(turn.start, turn.end, speaker)
             for turn, _, speaker in diarization.itertracks(yield_label=True)]

    # Map pyannote labels to friendly names
    unique_speakers = sorted(set(t[2] for t in turns))
    speaker_map = {s: f"Speaker {i+1}" for i, s in enumerate(unique_speakers)}

    for seg in segments:
        seg_mid = (seg["start"] + seg["end"]) / 2
        best_speaker = None
        best_overlap = 0
        for t_start, t_end, t_speaker in turns:
            overlap = max(0, min(seg["end"], t_end) - max(seg["start"], t_start))
            if overlap > best_overlap:
                best_overlap = overlap
                best_speaker = t_speaker
        seg["speaker"] = speaker_map.get(best_speaker, "Unknown") if best_speaker else "Unknown"

    return segments
```

**Tests** (`tests/test_diarize.py`):

These tests should mock the pyannote Pipeline to avoid requiring the actual model.

1. `test_merge_speakers_basic` — Two speakers, segments correctly assigned
2. `test_merge_speakers_maps_labels` — Verifies `SPEAKER_00` → `Speaker 1`, `SPEAKER_01` → `Speaker 2`
3. `test_merge_speakers_no_overlap` — Segment with no diarization overlap gets `Unknown`
4. `test_merge_speakers_overlap_resolution` — When segment spans two speaker turns, picks the one with more overlap

For mocking, create a fake diarization object that supports `itertracks(yield_label=True)`:
```python
from unittest.mock import MagicMock
from pyannote.core import Segment as PyannoteSegment, Annotation

def make_mock_diarization(turns):
    """Create a real pyannote Annotation from a list of (start, end, speaker) tuples."""
    annotation = Annotation()
    for start, end, speaker in turns:
        annotation[PyannoteSegment(start, end)] = speaker
    return annotation
```

**Commit message:** `feat: add speaker diarization with pyannote`

---

## Task 3: Update segment-transcript.py to handle speaker labels

**Changes to `segment-transcript.py`:**

The `segment_by_tickets` function currently joins segment texts with spaces. Update it to include speaker labels when present.

**Current behavior** (line 46):
```python
segment_texts.append(seg["text"].strip())
```

**New behavior:**
```python
text = seg["text"].strip()
speaker = seg.get("speaker")
if speaker:
    segment_texts.append(f"**{speaker}:** {text}")
else:
    segment_texts.append(text)
```

And change the join from space to double-newline so each speaker turn is on its own line:
```python
# Old
"text": " ".join(segment_texts),
# New
"text": "\n\n".join(segment_texts),
```

**Example .md output after this change:**
```markdown
# GT-9516 - Meeting Transcript

**Source:** 2026-02-19-100000/audio.wav
**Date:** 2026-02-19
**Segment:** 00:00:00 - 00:08:42

---

**Speaker 1:** So this one is about the freemium BYO experience.

**Speaker 2:** Yeah, we want to A/B test three variations.

**Speaker 1:** Right, and we'll use LaunchDarkly for the flag.
```

**Update existing tests:**

The existing tests in `tests/test_segment_transcript.py` use segments without speaker labels, so they should continue to pass (the `seg.get("speaker")` returns None, falls back to no-label behavior).

**Add new tests:**

1. `test_segment_by_tickets_with_speakers` — Segments have speaker labels, output includes `**Speaker 1:**` prefixes
2. `test_segment_by_tickets_mixed_speakers_and_no_speakers` — Mix of labeled and unlabeled segments

**Commit message:** `feat: include speaker labels in transcript output`

---

## Task 4: Update process-local-recordings.sh to call diarize.py

**Changes to `process-local-recordings.sh`:**

After whisper produces its JSON output, run `diarize.py` to add speaker labels before passing to `segment-transcript.py`.

Add `HF_TOKEN` loading from `.env` and pass it to diarize.py.

**Current flow:**
```
whisper-wrapper.py → whisper JSON → segment-transcript.py
```

**New flow:**
```
whisper-wrapper.py → whisper JSON → diarize.py → enriched JSON → segment-transcript.py
```

In the bash script, after the whisper output is saved to the temp file:
```bash
# Diarize — add speaker labels
echo "  Running speaker diarization..."
diarized_output_file=$(mktemp /tmp/diarized-output-XXXXXX.json)
"$PYTHON_ENV" "$DIARIZE_SCRIPT" "$audio_file" "$whisper_output_file" ${HF_TOKEN:+--hf-token "$HF_TOKEN"} > "$diarized_output_file"

# Use diarized output for segmentation (fall back to whisper output if diarization fails)
```

Add a fallback: if diarization fails (non-zero exit), log a warning and use the original whisper output without speaker labels.

**Also update:**
- `.env.example` — add `HF_TOKEN=your-huggingface-token-here`
- Load `HF_TOKEN` from `.env` in the script

**Commit message:** `feat: integrate diarization into pipeline`

---

## Task 5: Run all tests

Run the full test suites for both projects:
```bash
cd /home/zknowles/personal/claude-tools/meeting-transcripts && python3 -m pytest tests/ -v
cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/ -v
```

All tests must pass before proceeding.

---

## Task 6: Code review

Dispatch a code review agent to review all changes against this plan. Check for:
1. Correct merging algorithm (overlap-based speaker assignment)
2. Graceful fallback when diarization fails
3. Backward compatibility (segments without speaker labels still work)
4. Test coverage of the merging logic
5. No regressions in existing tests

---

## Task 7: Manual end-to-end test

Test using an existing small recording from the project. Steps:

1. Create a test zoom-recorder output directory:
```bash
mkdir -p ~/zoom-recordings/2026-02-20-100000
ffmpeg -i /home/zknowles/personal/claude-tools/meeting-transcripts/recordings/GT-9523-GT-9524.mp4 \
    -ar 16000 -ac 1 -y ~/zoom-recordings/2026-02-20-100000/audio.wav
```

2. Create a timestamps.json with a single ticket mark:
```json
{"marks": [{"time": "00:00:00", "seconds": 0, "ticket": "GT-9523", "note": null}]}
```

3. Create a metadata.json.

4. Run the pipeline:
```bash
cd /home/zknowles/personal/claude-tools/meeting-transcripts
./process-local-recordings.sh 2026-02-20 /tmp/test-diarization-output
```

5. Verify the output `.md` file contains speaker labels (`**Speaker 1:**`, `**Speaker 2:**`, etc.)

6. Clean up the test data.

7. Report the results — show the first ~20 lines of the output `.md` file to confirm speaker labels are present and make sense.

---

## File Summary

| File | Action | Description |
|------|--------|-------------|
| `diarize.py` | Create | Speaker diarization + merge with whisper segments |
| `tests/test_diarize.py` | Create | Tests for merging algorithm |
| `segment-transcript.py` | Modify | Include speaker labels in .md output |
| `tests/test_segment_transcript.py` | Modify | Add tests for speaker label handling |
| `process-local-recordings.sh` | Modify | Add diarization step between whisper and segmentation |
| `.env.example` | Modify | Add HF_TOKEN |
