# tests/test_recorder.py
import os
import subprocess
import tempfile
from unittest.mock import patch, MagicMock

def test_recorder_creates_output_directory():
    """Test that starting a recording creates the output directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_popen.return_value = MagicMock()
            mock_popen.return_value.poll.return_value = None

            from src.recorder import Recorder
            recorder = Recorder(
                output_base=tmpdir,
                mic_source='test_mic',
                monitor_source='test_monitor'
            )
            recorder.start()

            # Should have created a timestamped directory
            dirs = os.listdir(tmpdir)
            assert len(dirs) == 1
            assert os.path.isdir(os.path.join(tmpdir, dirs[0]))

            recorder.stop()


def test_recorder_stop_terminates_ffmpeg():
    """Test that stopping a recording terminates the FFmpeg process."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(
                output_base=tmpdir,
                mic_source='test_mic',
                monitor_source='test_monitor'
            )
            recorder.start()
            recorder.stop()

            mock_process.terminate.assert_called_once()


def test_recorder_is_recording_property():
    """Test the is_recording property reflects state."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(
                output_base=tmpdir,
                mic_source='test_mic',
                monitor_source='test_monitor'
            )

            assert recorder.is_recording is False
            recorder.start()
            assert recorder.is_recording is True
            recorder.stop()
            assert recorder.is_recording is False


def test_recorder_stop_uses_kill_on_timeout():
    """Test that stop() uses SIGKILL if terminate times out."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_process.wait.side_effect = [subprocess.TimeoutExpired('ffmpeg', 5), None]
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(
                output_base=tmpdir,
                mic_source='test_mic',
                monitor_source='test_monitor'
            )
            recorder.start()
            recorder.stop()

            mock_process.terminate.assert_called_once()
            mock_process.kill.assert_called_once()
