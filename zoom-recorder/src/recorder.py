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
