"""Timestamp management for recording marks."""

import json
from datetime import datetime


class TimestampManager:
    """Manages timestamp marks during a recording session."""

    def __init__(self, start_time: datetime):
        self.start_time = start_time
        self._marks = []

    def add_mark(self, elapsed_seconds: int, ticket: str | None = None, note: str | None = None) -> dict:
        """Add a timestamp mark."""
        hours = elapsed_seconds // 3600
        minutes = (elapsed_seconds % 3600) // 60
        seconds = elapsed_seconds % 60
        time_str = f"{hours:02d}:{minutes:02d}:{seconds:02d}"

        mark = {
            'time': time_str,
            'seconds': elapsed_seconds,
            'ticket': ticket,
            'note': note
        }
        self._marks.append(mark)
        return mark

    def get_marks(self) -> list[dict]:
        """Return all marks."""
        return self._marks.copy()

    def save(self, filepath: str) -> None:
        """Save marks to JSON file."""
        data = {'marks': self._marks}
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2)
