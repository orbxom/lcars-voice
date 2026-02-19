#!/usr/bin/env python3
"""Tests for segment-transcript.py"""

import json
import os
import sys
import tempfile

# Add parent dir to path so we can import the script
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def test_segment_by_tickets_basic():
    """Test basic segmentation with two ticket marks."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    segments = [
        {"start": 0.0, "end": 5.0, "text": "Hello everyone."},
        {"start": 5.0, "end": 10.0, "text": "Let's discuss the first ticket."},
        {"start": 10.0, "end": 15.0, "text": "Moving on to the second ticket."},
        {"start": 15.0, "end": 20.0, "text": "This needs a database change."},
    ]
    marks = [
        {"time": "00:00:00", "seconds": 0, "ticket": "GT-100", "note": None},
        {"time": "00:00:10", "seconds": 10, "ticket": "GT-200", "note": None},
    ]

    result = mod.segment_by_tickets(segments, marks)

    assert len(result) == 2
    assert result[0]["ticket"] == "GT-100"
    assert "Hello everyone" in result[0]["text"]
    assert "first ticket" in result[0]["text"]
    assert result[1]["ticket"] == "GT-200"
    assert "second ticket" in result[1]["text"]
    assert "database change" in result[1]["text"]


def test_segment_by_tickets_no_marks():
    """Test with no marks — returns full transcript."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    segments = [
        {"start": 0.0, "end": 5.0, "text": "Hello."},
        {"start": 5.0, "end": 10.0, "text": "Goodbye."},
    ]

    result = mod.segment_by_tickets(segments, [])

    assert len(result) == 1
    assert result[0]["ticket"] is None
    assert "Hello" in result[0]["text"]
    assert "Goodbye" in result[0]["text"]


def test_segment_by_tickets_content_before_first_mark():
    """Test that content before the first mark is included with the first ticket."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    segments = [
        {"start": 0.0, "end": 5.0, "text": "Welcome to the meeting."},
        {"start": 5.0, "end": 10.0, "text": "Some chit chat."},
        {"start": 60.0, "end": 65.0, "text": "Now about GT-100."},
        {"start": 120.0, "end": 125.0, "text": "Now about GT-200."},
    ]
    marks = [
        {"time": "00:01:00", "seconds": 60, "ticket": "GT-100", "note": None},
        {"time": "00:02:00", "seconds": 120, "ticket": "GT-200", "note": None},
    ]

    result = mod.segment_by_tickets(segments, marks)

    assert len(result) == 2
    # Content before first mark (0-60s) should be included with GT-100
    assert "Welcome" in result[0]["text"]
    assert "chit chat" in result[0]["text"]
    assert "GT-100" in result[0]["text"]
    assert result[0]["ticket"] == "GT-100"
    assert result[0]["start_time"] == "00:00:00"


def test_segment_by_tickets_marks_without_tickets():
    """Test that marks without ticket values are ignored."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    segments = [
        {"start": 0.0, "end": 5.0, "text": "Hello."},
    ]
    marks = [
        {"time": "00:00:00", "seconds": 0, "ticket": None, "note": None},
    ]

    result = mod.segment_by_tickets(segments, marks)

    assert len(result) == 1
    assert result[0]["ticket"] is None


def test_write_transcript_md_creates_file():
    """Test that write_transcript_md creates a properly formatted file."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "GT-100.md")
        mod.write_transcript_md(
            filepath=filepath,
            ticket="GT-100",
            source="2026-02-19-093015/audio.wav",
            date="2026-02-19",
            start_time="00:00:00",
            end_time="00:05:00",
            text="This is the transcript text.",
            append=False,
        )

        with open(filepath) as f:
            content = f.read()

        assert "# GT-100 - Meeting Transcript" in content
        assert "**Source:** 2026-02-19-093015/audio.wav" in content
        assert "**Date:** 2026-02-19" in content
        assert "**Segment:** 00:00:00 - 00:05:00" in content
        assert "This is the transcript text." in content


def test_write_transcript_md_appends():
    """Test that write_transcript_md appends to existing file."""
    from importlib.util import spec_from_file_location, module_from_spec
    spec = spec_from_file_location("segment_transcript",
        os.path.join(os.path.dirname(__file__), "..", "segment-transcript.py"))
    mod = module_from_spec(spec)
    spec.loader.exec_module(mod)

    with tempfile.TemporaryDirectory() as tmpdir:
        filepath = os.path.join(tmpdir, "GT-100.md")

        # Create initial file
        mod.write_transcript_md(
            filepath=filepath, ticket="GT-100",
            source="session1/audio.wav", date="2026-02-19",
            start_time="00:00:00", end_time="00:05:00",
            text="First session content.", append=False,
        )

        # Append second session
        mod.write_transcript_md(
            filepath=filepath, ticket="GT-100",
            source="session2/audio.wav", date="2026-02-19",
            start_time="00:02:00", end_time=None,
            text="Second session content.", append=True,
        )

        with open(filepath) as f:
            content = f.read()

        assert "First session content." in content
        assert "Second session content." in content
        assert "session2/audio.wav" in content
        assert content.count("---") >= 3  # separators
