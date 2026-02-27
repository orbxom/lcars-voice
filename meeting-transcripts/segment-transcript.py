#!/usr/bin/env python3
"""Segment a whisper transcript by timestamp marks and write per-ticket markdown files."""

import argparse
import json
import os
import sys


def load_json(path):
    with open(path) as f:
        return json.load(f)


def segment_by_tickets(segments, marks):
    """Split whisper segments into per-ticket groups based on timestamp marks.

    Each mark defines the start of a ticket's discussion. Segments are assigned
    to whichever ticket's mark they fall under (by segment start time).

    Returns list of dicts: [{"ticket": str, "start_time": str, "end_time": str|None, "text": str}]
    """
    # Filter to marks that have tickets
    ticket_marks = [m for m in marks if m.get("ticket")]

    if not ticket_marks:
        # No ticket marks — return full transcript as a single unmarked block
        has_speakers = any(seg.get("speaker") for seg in segments)
        if has_speakers:
            parts = []
            for seg in segments:
                text = seg["text"].strip()
                speaker = seg.get("speaker")
                if speaker:
                    parts.append(f"**{speaker}:** {text}")
                else:
                    parts.append(text)
            full_text = "\n\n".join(parts)
        else:
            full_text = " ".join(seg["text"].strip() for seg in segments)
        return [{"ticket": None, "start_time": None, "end_time": None, "text": full_text}]

    result = []
    for i, mark in enumerate(ticket_marks):
        # Content before the first mark belongs to the first ticket
        range_start = 0 if i == 0 else mark["seconds"]

        # End is the start of the next mark, or infinity for the last
        if i + 1 < len(ticket_marks):
            range_end = ticket_marks[i + 1]["seconds"]
        else:
            range_end = float("inf")

        # Collect whisper segments that fall in this range
        segment_texts = []
        for seg in segments:
            if range_start <= seg["start"] < range_end:
                text = seg["text"].strip()
                speaker = seg.get("speaker")
                if speaker:
                    segment_texts.append(f"**{speaker}:** {text}")
                else:
                    segment_texts.append(text)

        start_display = "00:00:00" if i == 0 else mark["time"]
        end_display = ticket_marks[i + 1]["time"] if i + 1 < len(ticket_marks) else None

        has_speakers = any(seg.get("speaker") for seg in segments
                           if range_start <= seg["start"] < range_end)
        joiner = "\n\n" if has_speakers else " "

        result.append({
            "ticket": mark["ticket"],
            "start_time": start_display,
            "end_time": end_display,
            "text": joiner.join(segment_texts),
        })

    return result


def write_transcript_md(filepath, ticket, source, date, start_time, end_time, text, append=False):
    """Write or append a transcript section to a markdown file."""
    if append and os.path.exists(filepath):
        with open(filepath, "a") as f:
            f.write("\n\n---\n\n")
            f.write(f"**Source:** {source}\n")
            segment_label = f"{start_time} - {end_time}" if end_time else f"{start_time} - end"
            f.write(f"**Segment:** {segment_label}\n")
            f.write(f"\n---\n\n{text}\n")
    else:
        with open(filepath, "w") as f:
            f.write(f"# {ticket} - Meeting Transcript\n\n")
            f.write(f"**Source:** {source}\n")
            f.write(f"**Date:** {date}\n")
            if start_time:
                segment_label = f"{start_time} - {end_time}" if end_time else f"{start_time} - end"
                f.write(f"**Segment:** {segment_label}\n")
            f.write(f"\n---\n\n{text}\n")


def main():
    parser = argparse.ArgumentParser(description="Segment whisper transcript by timestamp marks")
    parser.add_argument("whisper_json", help="Path to whisper JSON output file")
    parser.add_argument("output_dir", help="Directory to write per-ticket .md files")
    parser.add_argument("timestamps_json", nargs="?", default=None,
                        help="Path to timestamps.json (optional, for backward compat with old recordings)")
    parser.add_argument("--source", required=True, help="Source identifier for the .md header (e.g. '2026-02-19-093015/audio.wav')")
    parser.add_argument("--date", required=True, help="Recording date (e.g. '2026-02-19')")
    args = parser.parse_args()

    whisper_data = load_json(args.whisper_json)

    marks = []
    if args.timestamps_json and os.path.exists(args.timestamps_json):
        timestamps_data = load_json(args.timestamps_json)
        marks = timestamps_data.get("marks", [])

    segments = whisper_data.get("segments", [])

    if not segments:
        print("Warning: No segments in whisper output", file=sys.stderr)
        return

    ticket_groups = segment_by_tickets(segments, marks)

    os.makedirs(args.output_dir, exist_ok=True)

    for group in ticket_groups:
        ticket = group["ticket"]
        if not ticket:
            # No ticket — use a generic name based on source
            source_base = os.path.basename(os.path.dirname(args.source)) if "/" in args.source else args.source
            ticket = source_base

        filepath = os.path.join(args.output_dir, f"{ticket}.md")
        append = os.path.exists(filepath)

        write_transcript_md(
            filepath=filepath,
            ticket=ticket,
            source=args.source,
            date=args.date,
            start_time=group["start_time"],
            end_time=group["end_time"],
            text=group["text"],
            append=append,
        )
        action = "Appended to" if append else "Created"
        print(f"  {action} {filepath}")


if __name__ == "__main__":
    main()
