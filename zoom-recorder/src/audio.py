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
