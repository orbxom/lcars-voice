"""Recording metadata writer."""

import json
from datetime import datetime


def write_metadata(filepath: str, start_time: datetime, end_time: datetime,
                    recording_duration: float | None = None) -> None:
    """Write recording metadata to JSON file.

    Args:
        recording_duration: Actual recording time in seconds (excluding pauses).
            If None, computed from end_time - start_time.
    """
    wall_duration = int((end_time - start_time).total_seconds())

    data = {
        'started_at': start_time.isoformat(),
        'ended_at': end_time.isoformat(),
        'duration_seconds': int(recording_duration) if recording_duration is not None else wall_duration,
        'wall_duration_seconds': wall_duration,
        'sample_rate': 16000,
        'channels': 1,
        'format': 'wav'
    }

    with open(filepath, 'w') as f:
        json.dump(data, f, indent=2)
