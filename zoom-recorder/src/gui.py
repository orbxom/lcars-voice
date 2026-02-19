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
            if messagebox.askyesno("Stop Recording", "Stop recording and save?"):
                self._stop_recording()
        else:
            self._start_recording()

    def _start_recording(self):
        """Start a new recording session."""
        try:
            self.recorder = Recorder(
                output_base=self.output_base,
                mic_source=self.mic_source,
                monitor_source=self.monitor_source
            )
            self.recorder.start()
        except Exception as e:
            messagebox.showerror("Error", f"Failed to start recording:\n{e}")
            return

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

        try:
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
            messagebox.showinfo("Recording Saved", f"Saved to:\n{output_dir}")
        except Exception as e:
            messagebox.showerror("Error", f"Error saving recording:\n{e}")
        finally:
            self.record_btn.configure(text="Start Recording")
            self.mark_btn.configure(state=tk.DISABLED)
            self.rec_indicator.configure(text="")
            self.timer_label.configure(text="00:00:00")

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
