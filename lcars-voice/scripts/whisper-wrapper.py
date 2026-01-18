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
        print(json.dumps({
            "text": result["text"].strip(),
            "language": result.get("language", "en")
        }))
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
