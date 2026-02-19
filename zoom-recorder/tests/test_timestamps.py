import json
import os
import tempfile
from datetime import datetime, timedelta

def test_timestamp_manager_add_mark():
    """Test adding timestamp marks."""
    from src.timestamps import TimestampManager

    start_time = datetime.now()
    mgr = TimestampManager(start_time)

    # Simulate 2 minutes 15 seconds elapsed
    mgr.add_mark(elapsed_seconds=135, ticket="GT-1234")

    marks = mgr.get_marks()
    assert len(marks) == 1
    assert marks[0]['seconds'] == 135
    assert marks[0]['time'] == "00:02:15"
    assert marks[0]['ticket'] == "GT-1234"


def test_timestamp_manager_mark_without_ticket():
    """Test adding mark without JIRA ticket."""
    from src.timestamps import TimestampManager

    mgr = TimestampManager(datetime.now())
    mgr.add_mark(elapsed_seconds=60, ticket=None)

    marks = mgr.get_marks()
    assert marks[0]['ticket'] is None


def test_timestamp_manager_save_json():
    """Test saving timestamps to JSON file."""
    with tempfile.TemporaryDirectory() as tmpdir:
        from src.timestamps import TimestampManager

        mgr = TimestampManager(datetime.now())
        mgr.add_mark(elapsed_seconds=60, ticket="GT-100")
        mgr.add_mark(elapsed_seconds=120, ticket="GT-200")

        filepath = os.path.join(tmpdir, "timestamps.json")
        mgr.save(filepath)

        with open(filepath) as f:
            data = json.load(f)

        assert len(data['marks']) == 2
        assert data['marks'][0]['ticket'] == "GT-100"
        assert data['marks'][1]['ticket'] == "GT-200"
