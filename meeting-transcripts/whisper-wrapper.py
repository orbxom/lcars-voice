#!/usr/bin/env python3
"""Simple whisper wrapper that outputs transcription to stdout."""

import sys
import whisper
import json

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "No audio file provided"}))
        sys.exit(1)

    audio_path = sys.argv[1]
    model_name = sys.argv[2] if len(sys.argv) > 2 else "base"

    try:
        # Use CUDA if available for faster transcription
        import torch
        device = "cuda" if torch.cuda.is_available() else "cpu"
        print(f"[WHISPER] Using device: {device}", file=sys.stderr)
        if device == "cuda":
            print(f"[WHISPER] GPU: {torch.cuda.get_device_name(0)}", file=sys.stderr)
        model = whisper.load_model(model_name, device=device)
        result = model.transcribe(audio_path, language="en", fp16=(device == "cuda"))
        output = {
            "text": result["text"].strip(),
            "language": result.get("language", "en"),
            "segments": [
                {"start": seg["start"], "end": seg["end"], "text": seg["text"],
                 "no_speech_prob": seg.get("no_speech_prob", 0.0),
                 "avg_logprob": seg.get("avg_logprob", 0.0)}
                for seg in result.get("segments", [])
            ]
        }
        print(json.dumps(output))
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
