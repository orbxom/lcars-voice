# tests/test_audio.py
import subprocess
from unittest.mock import patch

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
