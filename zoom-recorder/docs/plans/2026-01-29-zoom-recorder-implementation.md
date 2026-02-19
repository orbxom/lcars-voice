# Zoom Audio Recorder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Python/Tkinter tool to record Zoom audio with JIRA timestamp marking.

**Architecture:** Single Python script with Tkinter GUI controlling an FFmpeg subprocess. Audio captured via PulseAudio sources (mic + system monitor), mixed in real-time. Timestamps stored in JSON alongside WAV output.

**Tech Stack:** Python 3 (stdlib only), Tkinter, FFmpeg, PulseAudio/PipeWire

---

### Task 1: Audio Source Detection Module

**Files:**
- Create: `src/audio.py`
- Test: `tests/test_audio.py`

**Step 1: Write the failing test**

```python
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
```

**Step 2: Run test to verify it fails**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_audio.py -v`
Expected: FAIL with "No module named 'src'"

**Step 3: Write minimal implementation**

```python
# src/__init__.py
# (empty file to make src a package)
```

```python
# src/audio.py
"""Audio source detection for PipeWire/PulseAudio."""

import subprocess


def detect_sources() -> dict:
    """Detect available audio sources using pactl.

    Returns:
        dict with 'mic' and 'monitor' keys, values are source names or None
    """
    result = subprocess.run(
        ['pactl', 'list', 'sources', 'short'],
        capture_output=True,
        text=True
    )

    mic = None
    monitor = None

    for line in result.stdout.strip().split('\n'):
        if not line:
            continue
        parts = line.split('\t')
        if len(parts) >= 2:
            source_name = parts[1]
            if '.monitor' in source_name and monitor is None:
                monitor = source_name
            elif '.monitor' not in source_name and mic is None:
                mic = source_name

    return {'mic': mic, 'monitor': monitor}
```

**Step 4: Run test to verify it passes**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_audio.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/__init__.py src/audio.py tests/test_audio.py
git commit -m "feat: add audio source detection module"
```

---

### Task 2: Recorder Class - Start/Stop FFmpeg

**Files:**
- Create: `src/recorder.py`
- Test: `tests/test_recorder.py`

**Step 1: Write the failing test**

```python
# tests/test_recorder.py
import os
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
```

**Step 2: Run test to verify it fails**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_recorder.py -v`
Expected: FAIL with "No module named 'src.recorder'"

**Step 3: Write minimal implementation**

```python
# src/recorder.py
"""Audio recorder using FFmpeg."""

import os
import subprocess
from datetime import datetime


class Recorder:
    """Records audio from mic and system monitor sources."""

    def __init__(self, output_base: str, mic_source: str, monitor_source: str):
        self.output_base = output_base
        self.mic_source = mic_source
        self.monitor_source = monitor_source
        self._process = None
        self._output_dir = None
        self._start_time = None

    @property
    def is_recording(self) -> bool:
        return self._process is not None and self._process.poll() is None

    @property
    def output_dir(self) -> str | None:
        return self._output_dir

    @property
    def start_time(self) -> datetime | None:
        return self._start_time

    def start(self) -> str:
        """Start recording. Returns the output directory path."""
        if self.is_recording:
            raise RuntimeError("Already recording")

        # Create timestamped output directory
        self._start_time = datetime.now()
        dirname = self._start_time.strftime("%Y-%m-%d-%H%M%S")
        self._output_dir = os.path.join(self.output_base, dirname)
        os.makedirs(self._output_dir, exist_ok=True)

        audio_path = os.path.join(self._output_dir, "audio.wav")

        # FFmpeg command to capture and mix mic + monitor
        cmd = [
            'ffmpeg',
            '-f', 'pulse', '-i', self.mic_source,
            '-f', 'pulse', '-i', self.monitor_source,
            '-filter_complex', 'amix=inputs=2:duration=longest',
            '-ar', '16000',
            '-ac', '1',
            '-y',
            audio_path
        ]

        self._process = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )

        return self._output_dir

    def stop(self) -> None:
        """Stop recording."""
        if self._process is None:
            return

        if self._process.poll() is None:
            self._process.terminate()
            self._process.wait(timeout=5)

        self._process = None
```

**Step 4: Run test to verify it passes**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_recorder.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/recorder.py tests/test_recorder.py
git commit -m "feat: add recorder class with FFmpeg subprocess control"
```

---

### Task 3: Timestamp Manager

**Files:**
- Create: `src/timestamps.py`
- Test: `tests/test_timestamps.py`

**Step 1: Write the failing test**

```python
# tests/test_timestamps.py
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
```

**Step 2: Run test to verify it fails**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_timestamps.py -v`
Expected: FAIL with "No module named 'src.timestamps'"

**Step 3: Write minimal implementation**

```python
# src/timestamps.py
"""Timestamp management for recording marks."""

import json
from datetime import datetime


class TimestampManager:
    """Manages timestamp marks during a recording session."""

    def __init__(self, start_time: datetime):
        self.start_time = start_time
        self._marks = []

    def add_mark(self, elapsed_seconds: int, ticket: str | None = None, note: str | None = None) -> dict:
        """Add a timestamp mark.

        Args:
            elapsed_seconds: Seconds since recording started
            ticket: Optional JIRA ticket number
            note: Optional free-text note

        Returns:
            The created mark dict
        """
        hours = elapsed_seconds // 3600
        minutes = (elapsed_seconds % 3600) // 60
        seconds = elapsed_seconds % 60
        time_str = f"{hours:02d}:{minutes:02d}:{seconds:02d}"

        mark = {
            'time': time_str,
            'seconds': elapsed_seconds,
            'ticket': ticket,
            'note': note
        }
        self._marks.append(mark)
        return mark

    def get_marks(self) -> list[dict]:
        """Return all marks."""
        return self._marks.copy()

    def save(self, filepath: str) -> None:
        """Save marks to JSON file."""
        data = {'marks': self._marks}
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2)
```

**Step 4: Run test to verify it passes**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_timestamps.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/timestamps.py tests/test_timestamps.py
git commit -m "feat: add timestamp manager for JIRA marks"
```

---

### Task 4: Metadata Writer

**Files:**
- Create: `src/metadata.py`
- Test: `tests/test_metadata.py`

**Step 1: Write the failing test**

```python
# tests/test_metadata.py
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
```

**Step 2: Run test to verify it fails**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_metadata.py -v`
Expected: FAIL with "No module named 'src.metadata'"

**Step 3: Write minimal implementation**

```python
# src/metadata.py
"""Recording metadata writer."""

import json
from datetime import datetime


def write_metadata(filepath: str, start_time: datetime, end_time: datetime) -> None:
    """Write recording metadata to JSON file.

    Args:
        filepath: Path to write metadata.json
        start_time: Recording start time
        end_time: Recording end time
    """
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
```

**Step 4: Run test to verify it passes**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_metadata.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add src/metadata.py tests/test_metadata.py
git commit -m "feat: add metadata writer"
```

---

### Task 5: GUI - Main Window Structure

**Files:**
- Create: `src/gui.py`
- Test: (manual testing - Tkinter difficult to unit test)

**Step 1: Create GUI skeleton**

```python
# src/gui.py
"""Tkinter GUI for Zoom Recorder."""

import tkinter as tk
from tkinter import ttk, messagebox
from datetime import datetime
import os

from src.audio import detect_sources
from src.recorder import Recorder
from src.timestamps import TimestampManager
from src.metadata import write_metadata


class ZoomRecorderApp:
    """Main application window."""

    def __init__(self, output_base: str = None):
        self.output_base = output_base or os.path.expanduser("~/zoom-recordings")

        # Detect audio sources
        sources = detect_sources()
        self.mic_source = sources['mic']
        self.monitor_source = sources['monitor']

        if not self.mic_source or not self.monitor_source:
            raise RuntimeError(
                f"Audio sources not detected.\n"
                f"Mic: {self.mic_source}\n"
                f"Monitor: {self.monitor_source}\n"
                f"Run setup.sh to configure audio."
            )

        self.recorder = None
        self.timestamp_mgr = None
        self._timer_id = None

        self._build_ui()

    def _build_ui(self):
        """Build the UI components."""
        self.root = tk.Tk()
        self.root.title("Zoom Recorder")
        self.root.geometry("300x220")
        self.root.resizable(False, False)
        self.root.attributes('-topmost', True)

        # Main frame with padding
        main = ttk.Frame(self.root, padding="10")
        main.pack(fill=tk.BOTH, expand=True)

        # Start/Stop button
        self.record_btn = ttk.Button(
            main, text="Start Recording", command=self._toggle_recording
        )
        self.record_btn.pack(fill=tk.X, pady=(0, 10))

        # Timer and recording indicator
        timer_frame = ttk.Frame(main)
        timer_frame.pack(fill=tk.X, pady=(0, 10))

        self.timer_label = ttk.Label(timer_frame, text="00:00:00", font=('Mono', 14))
        self.timer_label.pack(side=tk.LEFT)

        self.rec_indicator = ttk.Label(timer_frame, text="", foreground='red', font=('Mono', 14))
        self.rec_indicator.pack(side=tk.RIGHT)

        # JIRA ticket entry
        ttk.Label(main, text="JIRA Ticket:").pack(anchor=tk.W)
        self.ticket_var = tk.StringVar()
        self.ticket_entry = ttk.Entry(main, textvariable=self.ticket_var)
        self.ticket_entry.pack(fill=tk.X, pady=(0, 5))
        self.ticket_entry.bind('<Return>', lambda e: self._mark_timestamp())

        # Mark button
        self.mark_btn = ttk.Button(
            main, text="Mark Timestamp", command=self._mark_timestamp, state=tk.DISABLED
        )
        self.mark_btn.pack(fill=tk.X, pady=(0, 10))

        # Last mark display
        self.last_mark_label = ttk.Label(main, text="", foreground='gray')
        self.last_mark_label.pack(anchor=tk.W)

        # Handle window close
        self.root.protocol("WM_DELETE_WINDOW", self._on_close)

    def _toggle_recording(self):
        """Start or stop recording."""
        if self.recorder and self.recorder.is_recording:
            self._stop_recording()
        else:
            self._start_recording()

    def _start_recording(self):
        """Start a new recording session."""
        self.recorder = Recorder(
            output_base=self.output_base,
            mic_source=self.mic_source,
            monitor_source=self.monitor_source
        )
        self.recorder.start()
        self.timestamp_mgr = TimestampManager(self.recorder.start_time)

        self.record_btn.configure(text="Stop Recording")
        self.mark_btn.configure(state=tk.NORMAL)
        self.rec_indicator.configure(text="● REC")
        self.last_mark_label.configure(text="")

        self._update_timer()

    def _stop_recording(self):
        """Stop the current recording."""
        if self._timer_id:
            self.root.after_cancel(self._timer_id)
            self._timer_id = None

        end_time = datetime.now()
        self.recorder.stop()

        # Save timestamps and metadata
        output_dir = self.recorder.output_dir
        self.timestamp_mgr.save(os.path.join(output_dir, "timestamps.json"))
        write_metadata(
            os.path.join(output_dir, "metadata.json"),
            self.recorder.start_time,
            end_time
        )

        self.record_btn.configure(text="Start Recording")
        self.mark_btn.configure(state=tk.DISABLED)
        self.rec_indicator.configure(text="")
        self.timer_label.configure(text="00:00:00")

        messagebox.showinfo("Recording Saved", f"Saved to:\n{output_dir}")

    def _mark_timestamp(self):
        """Mark current timestamp with optional JIRA ticket."""
        if not self.recorder or not self.recorder.is_recording:
            return

        elapsed = int((datetime.now() - self.recorder.start_time).total_seconds())
        ticket = self.ticket_var.get().strip() or None

        mark = self.timestamp_mgr.add_mark(elapsed, ticket)

        display = f"Last: {mark['time']}"
        if ticket:
            display += f" → {ticket}"
        self.last_mark_label.configure(text=display)

        self.ticket_var.set("")

    def _update_timer(self):
        """Update the elapsed time display."""
        if not self.recorder or not self.recorder.is_recording:
            return

        elapsed = int((datetime.now() - self.recorder.start_time).total_seconds())
        hours = elapsed // 3600
        minutes = (elapsed % 3600) // 60
        seconds = elapsed % 60
        self.timer_label.configure(text=f"{hours:02d}:{minutes:02d}:{seconds:02d}")

        self._timer_id = self.root.after(1000, self._update_timer)

    def _on_close(self):
        """Handle window close."""
        if self.recorder and self.recorder.is_recording:
            if messagebox.askyesno("Recording Active", "Stop recording and save?"):
                self._stop_recording()
            else:
                return
        self.root.destroy()

    def run(self):
        """Start the application."""
        self.root.mainloop()
```

**Step 2: Create main entry point**

```python
# src/__main__.py
"""Entry point for zoom-recorder."""

from src.gui import ZoomRecorderApp


def main():
    try:
        app = ZoomRecorderApp()
        app.run()
    except RuntimeError as e:
        print(f"Error: {e}")
        exit(1)


if __name__ == "__main__":
    main()
```

**Step 3: Test manually**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m src`
Expected: GUI window appears with Start Recording button

**Step 4: Commit**

```bash
git add src/gui.py src/__main__.py
git commit -m "feat: add Tkinter GUI with recording controls"
```

---

### Task 6: Integration Test

**Files:**
- Test: `tests/test_integration.py`

**Step 1: Write integration test**

```python
# tests/test_integration.py
"""Integration tests for the recorder workflow."""

import json
import os
import tempfile
import time
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
```

**Step 2: Run integration test**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/test_integration.py -v`
Expected: PASS

**Step 3: Commit**

```bash
git add tests/test_integration.py
git commit -m "test: add integration test for recording workflow"
```

---

### Task 7: Create tests/__init__.py and Run All Tests

**Files:**
- Create: `tests/__init__.py`

**Step 1: Create test package init**

```python
# tests/__init__.py
# (empty file to make tests a package)
```

**Step 2: Run all tests**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m pytest tests/ -v`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add tests/__init__.py
git commit -m "chore: add tests package init"
```

---

### Task 8: Manual End-to-End Test

**Step 1: Run the application**

Run: `cd /home/zknowles/personal/claude-tools/zoom-recorder && python -m src`

**Step 2: Test workflow**

1. Click "Start Recording"
2. Verify timer starts counting
3. Enter "TEST-123" in JIRA field, press Enter
4. Verify "Last: 00:00:XX → TEST-123" appears
5. Mark another timestamp without ticket
6. Click "Stop Recording"
7. Verify dialog shows save path

**Step 3: Verify output**

Run: `ls -la ~/zoom-recordings/` and check latest directory contains:
- `audio.wav`
- `timestamps.json`
- `metadata.json`

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete zoom audio recorder v1"
```
