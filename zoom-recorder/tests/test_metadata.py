import json
import os
import tempfile
from datetime import datetime

def test_write_metadata():
    """Test writing recording metadata to JSON."""
    with tempfile.TemporaryDirectory() as tmpdir:
        from src.metadata import write_metadata

        start = datetime(2026, 1, 29, 14, 30, 22)
        end = datetime(2026, 1, 29, 15, 15, 45)

        filepath = os.path.join(tmpdir, "metadata.json")
        write_metadata(filepath, start, end)

        with open(filepath) as f:
            data = json.load(f)

        assert data['started_at'].startswith('2026-01-29T14:30:22')
        assert data['ended_at'].startswith('2026-01-29T15:15:45')
        assert data['duration_seconds'] == 2723
        assert data['sample_rate'] == 16000
        assert data['channels'] == 1
        assert data['format'] == 'wav'
