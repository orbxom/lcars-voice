"""Tests for diarize.py speaker diarization merging logic.

These tests mock the pyannote Pipeline so the actual model is not required.
Instead, they build real pyannote Annotation objects to exercise merge_speakers.
"""

import sys
import os

# Ensure the parent directory is importable.
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from pyannote.core import Segment as PyannoteSegment, Annotation

from diarize import merge_speakers, filter_hallucinations, merge_consecutive_speakers


def make_mock_diarization(turns):
    """Build a pyannote Annotation from a list of (start, end, speaker) tuples."""
    annotation = Annotation()
    for start, end, speaker in turns:
        annotation[PyannoteSegment(start, end)] = speaker
    return annotation


def test_merge_speakers_basic():
    """Two speakers, segments correctly assigned."""
    diarization = make_mock_diarization([
        (0.0, 5.0, "SPEAKER_00"),
        (5.0, 12.0, "SPEAKER_01"),
    ])
    segments = [
        {"start": 0.0, "end": 5.0, "text": " Hello everyone."},
        {"start": 5.0, "end": 12.0, "text": " Let's discuss the first ticket."},
    ]

    result = merge_speakers(segments, diarization)

    assert result[0]["speaker"] == "Speaker 1"
    assert result[1]["speaker"] == "Speaker 2"


def test_merge_speakers_maps_labels():
    """Verifies SPEAKER_00 -> Speaker 1, SPEAKER_01 -> Speaker 2."""
    diarization = make_mock_diarization([
        (0.0, 3.0, "SPEAKER_00"),
        (3.0, 6.0, "SPEAKER_01"),
        (6.0, 9.0, "SPEAKER_00"),
    ])
    segments = [
        {"start": 0.0, "end": 3.0, "text": " First."},
        {"start": 3.0, "end": 6.0, "text": " Second."},
        {"start": 6.0, "end": 9.0, "text": " Third."},
    ]

    result = merge_speakers(segments, diarization)

    assert result[0]["speaker"] == "Speaker 1"
    assert result[1]["speaker"] == "Speaker 2"
    assert result[2]["speaker"] == "Speaker 1"


def test_merge_speakers_no_overlap():
    """Segment with no diarization overlap gets nearest speaker (prev segment)."""
    diarization = make_mock_diarization([
        (0.0, 3.0, "SPEAKER_00"),
    ])
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Covered."},
        {"start": 10.0, "end": 15.0, "text": " No overlap at all."},
    ]

    result = merge_speakers(segments, diarization)

    assert result[0]["speaker"] == "Speaker 1"
    assert result[1]["speaker"] == "Speaker 1"  # assigned from previous segment


def test_merge_speakers_overlap_resolution():
    """When a segment spans two speaker turns, picks the one with more overlap."""
    # Speaker A: 0-4s, Speaker B: 4-10s
    # Segment spans 3-8s => 1s overlap with A, 4s overlap with B => B wins
    diarization = make_mock_diarization([
        (0.0, 4.0, "SPEAKER_00"),
        (4.0, 10.0, "SPEAKER_01"),
    ])
    segments = [
        {"start": 3.0, "end": 8.0, "text": " Spans both speakers."},
    ]

    result = merge_speakers(segments, diarization)

    assert result[0]["speaker"] == "Speaker 2"


# --- filter_hallucinations tests ---


def test_filter_hallucinations_high_no_speech_prob():
    """Segment with no_speech_prob=0.8 is removed."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Hello.", "no_speech_prob": 0.8, "avg_logprob": -0.3},
    ]
    result = filter_hallucinations(segments)
    assert len(result) == 0


def test_filter_hallucinations_low_logprob():
    """Segment with avg_logprob=-1.5 is removed."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Hello.", "no_speech_prob": 0.1, "avg_logprob": -1.5},
    ]
    result = filter_hallucinations(segments)
    assert len(result) == 0


def test_filter_hallucinations_non_latin():
    """Segment with Korean/CJK text is removed."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": "\uc548\ub155\ud558\uc138\uc694 \uc138\uacc4", "no_speech_prob": 0.1, "avg_logprob": -0.3},
    ]
    result = filter_hallucinations(segments)
    assert len(result) == 0


def test_filter_hallucinations_preserves_good_segments():
    """Normal segments pass through the filter."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Hello everyone.", "no_speech_prob": 0.1, "avg_logprob": -0.3},
        {"start": 3.0, "end": 6.0, "text": " How are you?", "no_speech_prob": 0.2, "avg_logprob": -0.5},
    ]
    result = filter_hallucinations(segments)
    assert len(result) == 2
    assert result[0]["text"] == " Hello everyone."
    assert result[1]["text"] == " How are you?"


def test_filter_hallucinations_no_confidence_fields():
    """Segments without confidence fields are preserved (backward compat)."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Old format segment."},
        {"start": 3.0, "end": 6.0, "text": " Another old segment."},
    ]
    result = filter_hallucinations(segments)
    assert len(result) == 2


# --- merge_consecutive_speakers tests ---


def test_merge_consecutive_speakers():
    """Three consecutive Speaker 1 segments become one merged segment."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Hello.", "speaker": "Speaker 1"},
        {"start": 3.0, "end": 6.0, "text": " How are you?", "speaker": "Speaker 1"},
        {"start": 6.0, "end": 9.0, "text": " I'm fine.", "speaker": "Speaker 1"},
    ]
    result = merge_consecutive_speakers(segments)
    assert len(result) == 1
    assert result[0]["speaker"] == "Speaker 1"
    assert result[0]["start"] == 0.0
    assert result[0]["end"] == 9.0
    assert result[0]["text"] == "Hello. How are you? I'm fine."


def test_merge_consecutive_speakers_different():
    """Alternating speakers are not merged."""
    segments = [
        {"start": 0.0, "end": 3.0, "text": " Hello.", "speaker": "Speaker 1"},
        {"start": 3.0, "end": 6.0, "text": " Hi there.", "speaker": "Speaker 2"},
        {"start": 6.0, "end": 9.0, "text": " Good morning.", "speaker": "Speaker 1"},
    ]
    result = merge_consecutive_speakers(segments)
    assert len(result) == 3
    assert result[0]["speaker"] == "Speaker 1"
    assert result[1]["speaker"] == "Speaker 2"
    assert result[2]["speaker"] == "Speaker 1"


# --- Unknown speaker assignment test ---


def test_unknown_assigned_to_nearest_speaker():
    """Segment that would be Unknown gets previous speaker's label."""
    # Speaker A covers 0-3s, nothing covers 10-15s
    diarization = make_mock_diarization([
        (0.0, 3.0, "SPEAKER_00"),
        (3.0, 6.0, "SPEAKER_01"),
    ])
    segments = [
        {"start": 0.0, "end": 3.0, "text": " First."},
        {"start": 3.0, "end": 6.0, "text": " Second."},
        {"start": 10.0, "end": 15.0, "text": " No overlap."},
    ]

    result = merge_speakers(segments, diarization)

    assert result[0]["speaker"] == "Speaker 1"
    assert result[1]["speaker"] == "Speaker 2"
    # The third segment has no overlap, so it should get the previous speaker
    assert result[2]["speaker"] == "Speaker 2"
