"""Integration tests for the recorder workflow."""

import json
import os
import tempfile
from unittest.mock import patch, MagicMock

def test_full_recording_workflow():
    """Test complete workflow: start, mark, stop, verify files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            from src.timestamps import TimestampManager
            from src.metadata import write_metadata
            from datetime import datetime

            # Start recording
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            output_dir = recorder.start()
            timestamp_mgr = TimestampManager(recorder.start_time)

            # Add marks
            timestamp_mgr.add_mark(60, "GT-100")
            timestamp_mgr.add_mark(120, "GT-200")
            timestamp_mgr.add_mark(180, None)

            # Stop and save
            end_time = datetime.now()
            recorder.stop()
            timestamp_mgr.save(os.path.join(output_dir, "timestamps.json"))
            write_metadata(
                os.path.join(output_dir, "metadata.json"),
                recorder.start_time,
                end_time
            )

            # Verify files exist
            assert os.path.exists(os.path.join(output_dir, "timestamps.json"))
            assert os.path.exists(os.path.join(output_dir, "metadata.json"))

            # Verify timestamps content
            with open(os.path.join(output_dir, "timestamps.json")) as f:
                ts_data = json.load(f)
            assert len(ts_data['marks']) == 3
            assert ts_data['marks'][0]['ticket'] == "GT-100"
            assert ts_data['marks'][2]['ticket'] is None

            # Verify metadata content
            with open(os.path.join(output_dir, "metadata.json")) as f:
                meta = json.load(f)
            assert meta['sample_rate'] == 16000
            assert meta['format'] == 'wav'
