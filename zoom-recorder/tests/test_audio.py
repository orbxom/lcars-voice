# tests/test_audio.py
import subprocess
from unittest.mock import patch
import pytest

def test_detect_sources_parses_pactl_output():
    """Test that we can parse pactl output to find mic and monitor sources."""
    mock_output = """0\talsa_input.usb-Audio-00.source\tPipeWire\ts16le 2ch 48000Hz\tIDLE
1\talsa_output.usb-Audio-00.sink.monitor\tPipeWire\ts16le 2ch 48000Hz\tIDLE
"""
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    assert sources['mic'] == 'alsa_input.usb-Audio-00.source'
    assert sources['monitor'] == 'alsa_output.usb-Audio-00.sink.monitor'


def test_detect_sources_returns_none_when_no_devices():
    """Test graceful handling when no audio devices found."""
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout="", stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    assert sources['mic'] is None
    assert sources['monitor'] is None


def test_detect_sources_raises_when_pactl_not_found():
    """Test that RuntimeError is raised when pactl is not installed."""
    with patch('subprocess.run') as mock_run:
        mock_run.side_effect = FileNotFoundError("pactl not found")
        from src.audio import detect_sources
        with pytest.raises(RuntimeError, match="pactl not found"):
            detect_sources()


def test_detect_sources_raises_on_timeout():
    """Test that RuntimeError is raised when pactl times out."""
    with patch('subprocess.run') as mock_run:
        mock_run.side_effect = subprocess.TimeoutExpired(cmd="pactl", timeout=5)
        from src.audio import detect_sources
        with pytest.raises(RuntimeError, match="timed out"):
            detect_sources()


def test_detect_sources_raises_on_nonzero_exit():
    """Test that RuntimeError is raised when pactl returns non-zero exit code."""
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=1, stdout="", stderr="Connection refused"
        )
        from src.audio import detect_sources
        with pytest.raises(RuntimeError, match="pactl failed with exit code 1"):
            detect_sources()
