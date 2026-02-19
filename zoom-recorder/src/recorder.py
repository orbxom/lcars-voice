"""Audio recorder using FFmpeg."""

import os
import subprocess
import time
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
        self._paused = False
        self._segments = []
        self._elapsed_before_pause = 0.0
        self._segment_start_time = None

    @property
    def is_recording(self) -> bool:
        return self._process is not None and self._process.poll() is None

    @property
    def is_paused(self) -> bool:
        return self._paused

    @property
    def output_dir(self) -> str | None:
        return self._output_dir

    @property
    def start_time(self) -> datetime | None:
        return self._start_time

    @property
    def elapsed_seconds(self) -> float:
        """Total recording time excluding paused periods."""
        total = self._elapsed_before_pause
        if self.is_recording and self._segment_start_time:
            total += (datetime.now() - self._segment_start_time).total_seconds()
        return total

    def start(self) -> str:
        """Start recording. Returns the output directory path."""
        if self.is_recording:
            raise RuntimeError("Already recording")

        # Create timestamped output directory
        self._start_time = datetime.now()
        dirname = self._start_time.strftime("%Y-%m-%d-%H%M%S")
        base_dir = os.path.join(self.output_base, dirname)
        self._output_dir = base_dir
        counter = 1
        while os.path.exists(self._output_dir):
            self._output_dir = f"{base_dir}-{counter}"
            counter += 1
        os.makedirs(self._output_dir)

        self._segments = []
        self._elapsed_before_pause = 0.0
        self._paused = False

        self._start_segment()
        return self._output_dir

    def pause(self) -> None:
        """Pause the current recording."""
        if self._paused:
            raise RuntimeError("Already paused")
        if not self.is_recording:
            raise RuntimeError("Not recording")

        # Accumulate elapsed time from this segment
        if self._segment_start_time:
            self._elapsed_before_pause += (datetime.now() - self._segment_start_time).total_seconds()
            self._segment_start_time = None

        self._paused = True
        self._stop_ffmpeg()

    def resume(self) -> None:
        """Resume a paused recording."""
        if not self._paused:
            raise RuntimeError("Not paused")

        self._paused = False
        try:
            self._start_segment()
        except Exception:
            self._paused = True
            raise

    def stop(self) -> None:
        """Stop recording and concatenate segments."""
        if self._process is None and not self._paused:
            return

        # Accumulate elapsed time from current segment
        if self.is_recording and self._segment_start_time:
            self._elapsed_before_pause += (datetime.now() - self._segment_start_time).total_seconds()
            self._segment_start_time = None

        self._stop_ffmpeg()
        self._paused = False

        if len(self._segments) > 1:
            self._concatenate_segments()
        elif len(self._segments) == 1:
            # Single segment — just rename to audio.wav
            final_path = os.path.join(self._output_dir, "audio.wav")
            if self._segments[0] != final_path and os.path.exists(self._segments[0]):
                os.rename(self._segments[0], final_path)

    def _start_segment(self) -> None:
        """Start a new FFmpeg segment."""
        segment_num = len(self._segments) + 1
        audio_path = os.path.join(self._output_dir, f"segment-{segment_num:03d}.wav")

        cmd = [
            'ffmpeg',
            '-f', 'pulse', '-i', self.mic_source,
            '-f', 'pulse', '-i', self.monitor_source,
            '-filter_complex', 'amix=inputs=2:duration=longest:normalize=0',
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

        # Check that FFmpeg actually started
        time.sleep(0.1)
        if self._process.poll() is not None:
            raise RuntimeError("FFmpeg failed to start")

        self._segments.append(audio_path)
        self._segment_start_time = datetime.now()

    def _stop_ffmpeg(self) -> None:
        """Terminate the current FFmpeg process."""
        if self._process is None:
            return

        if self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait()

        self._process = None

    def _concatenate_segments(self) -> None:
        """Concatenate all segment files into a single audio.wav."""
        concat_list_path = os.path.join(self._output_dir, "segments.txt")
        with open(concat_list_path, 'w') as f:
            for segment in self._segments:
                f.write(f"file '{os.path.basename(segment)}'\n")

        final_path = os.path.join(self._output_dir, "audio.wav")
        cmd = [
            'ffmpeg',
            '-f', 'concat',
            '-i', concat_list_path,
            '-c', 'copy',
            '-y',
            final_path
        ]

        result = subprocess.run(cmd, capture_output=True, timeout=30)
        if result.returncode != 0:
            raise RuntimeError(f"Failed to concatenate segments: {result.stderr.decode()}")

        # Clean up segment files and concat list
        for segment in self._segments:
            if os.path.exists(segment):
                os.remove(segment)
        os.remove(concat_list_path)
