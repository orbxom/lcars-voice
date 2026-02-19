"""Audio source detection for PipeWire/PulseAudio."""

import subprocess

# Known webcam identifiers to deprioritize as mic sources
_WEBCAM_PATTERNS = ['Brio', 'C920', 'C922', 'C930', 'Webcam', '046d_']


def _is_webcam(source_name: str) -> bool:
    """Check if a source name looks like a webcam rather than a real mic."""
    return any(pattern.lower() in source_name.lower() for pattern in _WEBCAM_PATTERNS)


def _pick_best(candidates: list[dict]) -> str | None:
    """Pick the best source from a list of candidates.

    Prefers RUNNING sources. Among RUNNING sources, deprioritizes webcams
    for mic selection. Falls back to the first available if none are RUNNING.
    """
    if not candidates:
        return None

    running = [c for c in candidates if c['state'] == 'RUNNING']
    if running:
        # Among RUNNING, prefer non-webcam sources
        non_webcam = [c for c in running if not _is_webcam(c['name'])]
        if non_webcam:
            return non_webcam[0]['name']
        return running[0]['name']

    return candidates[0]['name']


def detect_sources() -> dict:
    """Detect available audio sources using pactl.

    Prefers RUNNING sources over SUSPENDED/IDLE ones, since PipeWire marks
    the active default/in-use sources as RUNNING.

    Returns:
        dict with 'mic' and 'monitor' keys, values are source names or None

    Raises:
        RuntimeError: If pactl is not installed, times out, or fails
    """
    try:
        result = subprocess.run(
            ['pactl', 'list', 'sources', 'short'],
            capture_output=True,
            text=True,
            timeout=5
        )
    except FileNotFoundError:
        raise RuntimeError("pactl not found. Please install pulseaudio-utils or pipewire-pulse.")
    except subprocess.TimeoutExpired:
        raise RuntimeError("pactl command timed out after 5 seconds.")

    if result.returncode != 0:
        raise RuntimeError(f"pactl failed with exit code {result.returncode}: {result.stderr}")

    mic_candidates = []
    monitor_candidates = []

    for line in result.stdout.strip().split('\n'):
        if not line:
            continue
        parts = line.split('\t')
        if len(parts) >= 5:
            source_name = parts[1]
            state = parts[4].strip()
            entry = {'name': source_name, 'state': state}
            if '.monitor' in source_name:
                monitor_candidates.append(entry)
            else:
                mic_candidates.append(entry)
        elif len(parts) >= 2:
            # Fallback for lines without a state field
            source_name = parts[1]
            entry = {'name': source_name, 'state': 'UNKNOWN'}
            if '.monitor' in source_name:
                monitor_candidates.append(entry)
            else:
                mic_candidates.append(entry)

    return {
        'mic': _pick_best(mic_candidates),
        'monitor': _pick_best(monitor_candidates),
    }
