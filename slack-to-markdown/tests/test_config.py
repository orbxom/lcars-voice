import os
import pytest
from unittest.mock import patch
from src.config import load_config


def test_load_config_from_env():
    """Load token from environment variable."""
    with patch.dict(os.environ, {"SLACK_USER_TOKEN": "xoxp-test-token"}):
        config = load_config()
        assert config.token == "xoxp-test-token"


def test_load_config_missing_token_raises():
    """Missing token should raise with helpful message."""
    with patch.dict(os.environ, {}, clear=True):
        # Also ensure no .env file interferes
        with pytest.raises(SystemExit) as exc_info:
            load_config(env_file=None)
        assert exc_info.value.code == 1


def test_load_config_empty_token_raises():
    """Empty string token should raise."""
    with patch.dict(os.environ, {"SLACK_USER_TOKEN": ""}):
        with pytest.raises(SystemExit) as exc_info:
            load_config(env_file=None)
        assert exc_info.value.code == 1
