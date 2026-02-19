# tests/test_audio.py
import subprocess
from unittest.mock import patch
import pytest


def test_detect_sources_parses_pactl_output():
    """Test that we can parse pactl output to find mic and monitor sources."""
    mock_output = "0\talsa_input.usb-Audio-00.source\tPipeWire\ts16le 2ch 48000Hz\tIDLE\n1\talsa_output.usb-Audio-00.sink.monitor\tPipeWire\ts16le 2ch 48000Hz\tIDLE\n"
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


def test_detect_sources_prefers_running_over_suspended():
    """Test that RUNNING sources are preferred over SUSPENDED ones."""
    mock_output = (
        "54\talsa_output.usb-Generic_USB_Audio-00.HiFi__hw_Audio_3__sink.monitor\tPipeWire\ts16le 2ch 48000Hz\tSUSPENDED\n"
        "57\talsa_input.usb-Generic_USB_Audio-00.HiFi__hw_Audio_2__source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
        "59\talsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.analog-stereo.monitor\tPipeWire\ts24le 2ch 48000Hz\tRUNNING\n"
        "60\talsa_input.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.mono-fallback\tPipeWire\ts16le 1ch 32000Hz\tRUNNING\n"
    )
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    assert sources['mic'] == 'alsa_input.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.mono-fallback'
    assert sources['monitor'] == 'alsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.analog-stereo.monitor'


def test_detect_sources_deprioritizes_webcam():
    """Test that webcam sources are deprioritized among RUNNING mic candidates."""
    mock_output = (
        "59\talsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.analog-stereo.monitor\tPipeWire\ts24le 2ch 48000Hz\tRUNNING\n"
        "60\talsa_input.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.mono-fallback\tPipeWire\ts16le 1ch 32000Hz\tRUNNING\n"
        "61\talsa_input.usb-046d_Brio_100_2323LZ5070P8-02.mono-fallback\tPipeWire\ts16le 1ch 48000Hz\tRUNNING\n"
    )
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    # Should pick the HyperX, not the Brio webcam
    assert sources['mic'] == 'alsa_input.usb-HP__Inc_HyperX_Cloud_III_Wireless-00.mono-fallback'


def test_detect_sources_falls_back_when_none_running():
    """Test fallback to first source when no RUNNING sources exist."""
    mock_output = (
        "54\talsa_output.usb-Generic-00.sink.monitor\tPipeWire\ts16le 2ch 48000Hz\tSUSPENDED\n"
        "55\talsa_output.usb-Other-00.sink.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n"
        "57\talsa_input.usb-Generic-00.source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
        "58\talsa_input.usb-Other-00.source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
    )
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    # Falls back to first in the list
    assert sources['mic'] == 'alsa_input.usb-Generic-00.source'
    assert sources['monitor'] == 'alsa_output.usb-Generic-00.sink.monitor'


def test_detect_sources_full_realistic_output():
    """Test with the full realistic pactl output from the bug report."""
    mock_output = (
        "54\talsa_output.usb-Generic_USB_Audio-00.HiFi__hw_Audio_3__sink.monitor\tPipeWire\ts16le 2ch 48000Hz\tSUSPENDED\n"
        "55\talsa_output.usb-Generic_USB_Audio-00.HiFi__hw_Audio_1__sink.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n"
        "56\talsa_output.usb-Generic_USB_Audio-00.HiFi__hw_Audio__sink.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n"
        "57\talsa_input.usb-Generic_USB_Audio-00.HiFi__hw_Audio_2__source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
        "58\talsa_input.usb-Generic_USB_Audio-00.HiFi__hw_Audio_1__source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
        "59\talsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless_0000000000000000-00.analog-stereo.monitor\tPipeWire\ts24le 2ch 48000Hz\tRUNNING\n"
        "60\talsa_input.usb-HP__Inc_HyperX_Cloud_III_Wireless_0000000000000000-00.mono-fallback\tPipeWire\ts16le 1ch 32000Hz\tSUSPENDED\n"
        "61\talsa_input.usb-046d_Brio_100_2323LZ5070P8-02.mono-fallback\tPipeWire\ts16le 1ch 48000Hz\tSUSPENDED\n"
        "852\talsa_output.pci-0000_01_00.1.hdmi-surround.monitor\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n"
    )
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    # Monitor: the RUNNING HyperX monitor
    assert sources['monitor'] == 'alsa_output.usb-HP__Inc_HyperX_Cloud_III_Wireless_0000000000000000-00.analog-stereo.monitor'
    # Mic: no RUNNING mic, so falls back to first non-monitor source
    assert sources['mic'] == 'alsa_input.usb-Generic_USB_Audio-00.HiFi__hw_Audio_2__source'


def test_detect_sources_webcam_picked_if_only_running():
    """Test that a webcam is still picked if it's the only RUNNING mic source."""
    mock_output = (
        "57\talsa_input.usb-Generic-00.source\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
        "61\talsa_input.usb-046d_Brio_100-02.mono-fallback\tPipeWire\ts16le 1ch 48000Hz\tRUNNING\n"
        "59\talsa_output.usb-Audio-00.sink.monitor\tPipeWire\ts24le 2ch 48000Hz\tSUSPENDED\n"
    )
    with patch('subprocess.run') as mock_run:
        mock_run.return_value = subprocess.CompletedProcess(
            args=[], returncode=0, stdout=mock_output, stderr=""
        )
        from src.audio import detect_sources
        sources = detect_sources()

    # Webcam is still preferred over SUSPENDED generic source since it's RUNNING
    assert sources['mic'] == 'alsa_input.usb-046d_Brio_100-02.mono-fallback'
