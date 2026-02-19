#!/usr/bin/env python3
"""Standalone speaker diarization script.

Runs pyannote speaker diarization on an audio file, merges speaker labels
with existing whisper segments (from JSON), and outputs enriched JSON with
speaker labels added to each segment.

Usage:
    ~/voice-to-text-env/bin/python diarize.py <audio-file> <whisper-json-file> [--hf-token TOKEN]
"""

import argparse
import json
import os
import re
import sys
import unicodedata

from pyannote.audio import Pipeline


def _non_latin_ratio(text):
    """Return the fraction of characters in text that are non-Latin.

    Only counts actual letters/digits/symbols, skipping whitespace and
    punctuation so that short texts with spaces don't skew the ratio.
    """
    chars = [ch for ch in text if not ch.isspace()]
    if not chars:
        return 0.0
    non_latin = 0
    for ch in chars:
        # ASCII printable range (letters, digits, basic punctuation) is fine
        if ord(ch) <= 0x024F:
            continue
        # Latin Extended Additional / common accented Latin
        if 0x1E00 <= ord(ch) <= 0x1EFF:
            continue
        non_latin += 1
    return non_latin / len(chars)


def filter_hallucinations(segments):
    """Remove segments likely to be whisper hallucinations.

    Criteria:
      1. no_speech_prob > 0.6
      2. avg_logprob < -1.0
      3. More than 30% non-Latin characters

    Segments missing confidence fields are preserved for backward compatibility.

    Returns a new list (does not mutate the input).
    """
    kept = []
    removed_no_speech = 0
    removed_logprob = 0
    removed_non_latin = 0

    for seg in segments:
        # Backward compat: if no confidence fields, keep the segment
        has_confidence = "no_speech_prob" in seg or "avg_logprob" in seg

        if has_confidence and seg.get("no_speech_prob", 0.0) > 0.6:
            removed_no_speech += 1
            continue
        if has_confidence and seg.get("avg_logprob", 0.0) < -1.0:
            removed_logprob += 1
            continue
        if _non_latin_ratio(seg.get("text", "")) > 0.3:
            removed_non_latin += 1
            continue
        kept.append(seg)

    total_removed = removed_no_speech + removed_logprob + removed_non_latin
    if total_removed:
        print(
            f"[DIARIZE] Filtered {total_removed} hallucinated segment(s): "
            f"{removed_no_speech} high no_speech_prob, "
            f"{removed_logprob} low avg_logprob, "
            f"{removed_non_latin} non-Latin",
            file=sys.stderr,
        )

    return kept


def diarize(audio_path, hf_token=None):
    """Run pyannote speaker diarization on an audio file.

    Args:
        audio_path: Path to the audio file.
        hf_token: Optional HuggingFace auth token for model access.

    Returns:
        A pyannote Annotation object with speaker diarization results.
    """
    pipeline = Pipeline.from_pretrained(
        "pyannote/speaker-diarization-3.1",
        token=hf_token,
    )
    result = pipeline(audio_path)
    # pyannote 4.x returns DiarizeOutput; extract the Annotation object
    if hasattr(result, "speaker_diarization"):
        return result.speaker_diarization
    return result


def merge_speakers(segments, diarization):
    """Merge pyannote speaker labels into whisper segments.

    For each whisper segment, finds the pyannote turn with the greatest
    temporal overlap and assigns that turn's speaker label to the segment.

    Speaker labels are mapped from pyannote format (SPEAKER_00, SPEAKER_01)
    to human-readable format (Speaker 1, Speaker 2, etc.).

    Args:
        segments: List of whisper segment dicts with 'start', 'end', 'text'.
        diarization: A pyannote Annotation object.

    Returns:
        The segments list, with a 'speaker' key added to each segment.
    """
    turns = [
        (turn.start, turn.end, speaker)
        for turn, _, speaker in diarization.itertracks(yield_label=True)
    ]

    unique_speakers = sorted(set(t[2] for t in turns))
    speaker_map = {s: f"Speaker {i + 1}" for i, s in enumerate(unique_speakers)}

    for seg in segments:
        best_speaker = None
        best_overlap = 0
        for t_start, t_end, t_speaker in turns:
            overlap = max(0, min(seg["end"], t_end) - max(seg["start"], t_start))
            if overlap > best_overlap:
                best_overlap = overlap
                best_speaker = t_speaker
        seg["speaker"] = (
            speaker_map.get(best_speaker, "Unknown") if best_speaker else "Unknown"
        )

    # Assign Unknown segments to the nearest known speaker
    for i, seg in enumerate(segments):
        if seg["speaker"] == "Unknown":
            # Try previous segment first
            if i > 0 and segments[i - 1]["speaker"] != "Unknown":
                seg["speaker"] = segments[i - 1]["speaker"]
            # If first segment, try next segment
            elif i + 1 < len(segments) and segments[i + 1]["speaker"] != "Unknown":
                seg["speaker"] = segments[i + 1]["speaker"]

    return segments


def merge_consecutive_speakers(segments):
    """Merge adjacent segments that share the same speaker label.

    When consecutive segments have the same speaker, they are combined into
    a single segment with:
      - start = first segment's start
      - end = last segment's end
      - text = concatenation of all texts (space-joined, stripped)
      - speaker = the shared speaker label

    Confidence fields (no_speech_prob, avg_logprob) are dropped from output
    since they are only used for the hallucination filtering step.

    Returns a new list of merged segments.
    """
    if not segments:
        return []

    merged = []
    current = {
        "start": segments[0]["start"],
        "end": segments[0]["end"],
        "text": segments[0]["text"].strip(),
        "speaker": segments[0]["speaker"],
    }

    for seg in segments[1:]:
        if seg["speaker"] == current["speaker"]:
            current["end"] = seg["end"]
            current["text"] = current["text"] + " " + seg["text"].strip()
        else:
            merged.append(current)
            current = {
                "start": seg["start"],
                "end": seg["end"],
                "text": seg["text"].strip(),
                "speaker": seg["speaker"],
            }

    merged.append(current)
    return merged


def main():
    parser = argparse.ArgumentParser(
        description="Run speaker diarization and merge with whisper segments."
    )
    parser.add_argument("audio_file", help="Path to the audio file.")
    parser.add_argument("whisper_json_file", help="Path to the whisper JSON file.")
    parser.add_argument(
        "--hf-token",
        default=os.environ.get("HF_TOKEN"),
        help="HuggingFace auth token (defaults to $HF_TOKEN env var).",
    )
    args = parser.parse_args()

    with open(args.whisper_json_file, "r") as f:
        whisper_data = json.load(f)

    whisper_data["segments"] = filter_hallucinations(whisper_data["segments"])

    diarization = diarize(args.audio_file, hf_token=args.hf_token)
    merge_speakers(whisper_data["segments"], diarization)

    whisper_data["segments"] = merge_consecutive_speakers(whisper_data["segments"])

    json.dump(whisper_data, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
