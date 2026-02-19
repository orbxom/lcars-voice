"""Recording metadata writer."""

import json
from datetime import datetime


def write_metadata(filepath: str, start_time: datetime, end_time: datetime) -> None:
    """Write recording metadata to JSON file."""
    duration = int((end_time - start_time).total_seconds())

    data = {
        'started_at': start_time.isoformat(),
        'ended_at': end_time.isoformat(),
        'duration_seconds': duration,
        'sample_rate': 16000,
        'channels': 1,
        'format': 'wav'
    }

    with open(filepath, 'w') as f:
        json.dump(data, f, indent=2)
