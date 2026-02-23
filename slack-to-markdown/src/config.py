import os
import sys
from dataclasses import dataclass
from pathlib import Path

from dotenv import load_dotenv


@dataclass
class Config:
    token: str


def load_config(env_file: str | None = "auto") -> Config:
    """Load config from .env file and/or environment variables."""
    if env_file == "auto":
        dotenv_path = Path(__file__).parent.parent / ".env"
        if dotenv_path.exists():
            load_dotenv(dotenv_path)
    elif env_file is not None:
        load_dotenv(env_file)

    token = os.environ.get("SLACK_USER_TOKEN", "").strip()
    if not token:
        print(
            "Error: SLACK_USER_TOKEN not set.\n"
            "Copy .env.example to .env and add your Slack user token.\n"
            "Create a Slack app at https://api.slack.com/apps with scopes:\n"
            "  channels:history, channels:read, groups:history, groups:read,\n"
            "  im:history, mpim:history, users:read, files:read",
            file=sys.stderr,
        )
        sys.exit(1)

    return Config(token=token)
