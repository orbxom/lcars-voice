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


def test_recorder_start_raises_when_ffmpeg_fails():
    """Test that start() raises RuntimeError if FFmpeg exits immediately."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen, \
             patch('time.sleep'):
            mock_process = MagicMock()
            mock_process.poll.return_value = 1  # Already exited
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(
                output_base=tmpdir,
                mic_source='test_mic',
                monitor_source='test_monitor'
            )

            import pytest
            with pytest.raises(RuntimeError, match="FFmpeg failed to start"):
                recorder.start()


def test_recorder_start_raises_when_already_recording():
    """Test that start() raises RuntimeError if already recording."""
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

            import pytest
            with pytest.raises(RuntimeError, match="Already recording"):
                recorder.start()

            recorder.stop()


def test_recorder_stop_without_start():
    """Test that stop() is a no-op when not recording."""
    from src.recorder import Recorder
    recorder = Recorder(
        output_base='/tmp',
        mic_source='test_mic',
        monitor_source='test_monitor'
    )
    # Should not raise
    recorder.stop()


def test_pause_sets_paused_state():
    """Test that pause() sets is_paused and stops the FFmpeg process."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            assert recorder.is_paused is False
            recorder.pause()
            assert recorder.is_paused is True
            assert recorder.is_recording is False
            mock_process.terminate.assert_called_once()


def test_resume_clears_paused_state():
    """Test that resume() clears paused state and starts new segment."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()
            recorder.pause()
            recorder.resume()

            assert recorder.is_paused is False
            assert recorder.is_recording is True
            # Should have started FFmpeg twice (start + resume)
            assert mock_popen.call_count == 2


def test_pause_when_not_recording_raises():
    """Test that pause() raises when not recording."""
    import pytest
    from src.recorder import Recorder
    recorder = Recorder('/tmp', 'test_mic', 'test_monitor')

    with pytest.raises(RuntimeError, match="Not recording"):
        recorder.pause()


def test_pause_when_already_paused_raises():
    """Test that pause() raises when already paused."""
    import pytest
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()
            recorder.pause()

            with pytest.raises(RuntimeError, match="Already paused"):
                recorder.pause()


def test_resume_when_not_paused_raises():
    """Test that resume() raises when not paused."""
    import pytest
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            with pytest.raises(RuntimeError, match="Not paused"):
                recorder.resume()

            recorder.stop()


def test_stop_while_paused():
    """Test that stop() works correctly from paused state."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()
            recorder.pause()

            # Should not raise
            recorder.stop()
            assert recorder.is_paused is False
            assert recorder.is_recording is False


def test_stop_concatenates_multiple_segments():
    """Test that stop() calls ffmpeg concat when multiple segments exist."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen, \
             patch('subprocess.run') as mock_run:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process
            mock_run.return_value = MagicMock(returncode=0)

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            # Create fake segment files so cleanup works
            output_dir = recorder.output_dir
            seg1 = os.path.join(output_dir, "segment-001.wav")
            open(seg1, 'w').close()

            recorder.pause()
            recorder.resume()

            seg2 = os.path.join(output_dir, "segment-002.wav")
            open(seg2, 'w').close()

            recorder.stop()

            # Should have called ffmpeg concat
            mock_run.assert_called_once()
            call_args = mock_run.call_args[0][0]
            assert 'concat' in call_args


def test_stop_renames_single_segment():
    """Test that a single-segment recording renames to audio.wav."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            # Create the fake segment file
            output_dir = recorder.output_dir
            seg = os.path.join(output_dir, "segment-001.wav")
            with open(seg, 'w') as f:
                f.write("fake audio")

            recorder.stop()

            # Should have renamed to audio.wav
            assert os.path.exists(os.path.join(output_dir, "audio.wav"))
            assert not os.path.exists(seg)


def test_elapsed_seconds_excludes_pause():
    """Test that elapsed_seconds does not count paused periods."""
    from unittest.mock import PropertyMock
    from datetime import datetime, timedelta

    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen, \
             patch('src.recorder.datetime') as mock_dt:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process

            # Control time progression
            base = datetime(2026, 1, 1, 12, 0, 0)
            times = [
                base,                              # start() -> _start_time
                base + timedelta(seconds=10),      # _start_segment -> _segment_start_time
                base + timedelta(seconds=30),      # pause() -> elapsed calc (20s recorded)
                base + timedelta(seconds=60),      # resume() -> _start_segment -> _segment_start_time
                base + timedelta(seconds=80),      # elapsed_seconds check (20s more = 40 total)
                base + timedelta(seconds=80),      # stop() -> elapsed accumulation
            ]
            mock_dt.now = MagicMock(side_effect=times)
            mock_dt.side_effect = lambda *a, **k: datetime(*a, **k)

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            # After 20 seconds of recording, pause
            recorder.pause()
            # _elapsed_before_pause should be ~20 seconds
            assert 19 <= recorder.elapsed_seconds <= 21

            # Resume after 30-second pause
            recorder.resume()

            # After 20 more seconds recording, check elapsed
            # Should be ~40 total (20 + 20), not 80 wall clock
            elapsed = recorder.elapsed_seconds
            assert 39 <= elapsed <= 41


def test_multiple_pause_resume_cycles():
    """Test multiple pause/resume cycles maintain correct state."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen, \
             patch('subprocess.run') as mock_run:
            mock_process = MagicMock()
            mock_process.poll.return_value = None
            mock_popen.return_value = mock_process
            mock_run.return_value = MagicMock(returncode=0)

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()

            output_dir = recorder.output_dir
            for i in range(3):
                # Create fake segment file before pause
                seg = os.path.join(output_dir, f"segment-{i+1:03d}.wav")
                open(seg, 'w').close()

                recorder.pause()
                assert recorder.is_paused is True
                recorder.resume()
                assert recorder.is_recording is True

            # Create final segment file
            seg = os.path.join(output_dir, f"segment-{4:03d}.wav")
            open(seg, 'w').close()

            recorder.stop()

            # start + 3 resumes = 4 FFmpeg spawns
            assert mock_popen.call_count == 4
            # Should have called ffmpeg concat (4 segments)
            mock_run.assert_called_once()


def test_resume_failure_restores_paused_state():
    """Test that resume() restores paused state if FFmpeg fails to start."""
    import pytest
    with tempfile.TemporaryDirectory() as tmpdir:
        with patch('subprocess.Popen') as mock_popen, \
             patch('time.sleep'):
            # First call succeeds (start), second fails (resume)
            good_process = MagicMock()
            good_process.poll.return_value = None
            bad_process = MagicMock()
            bad_process.poll.return_value = 1  # FFmpeg exits immediately
            mock_popen.side_effect = [good_process, bad_process]

            from src.recorder import Recorder
            recorder = Recorder(tmpdir, 'test_mic', 'test_monitor')
            recorder.start()
            recorder.pause()

            with pytest.raises(RuntimeError, match="FFmpeg failed to start"):
                recorder.resume()

            # Should still be paused so stop() can clean up
            assert recorder.is_paused is True
