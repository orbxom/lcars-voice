# Slack-to-Markdown Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Python CLI tool that converts a Slack thread URL into a self-contained markdown file with downloaded images/attachments.

**Architecture:** Pure Python CLI using `slack_sdk` for API calls. Four modules: URL parser, Slack client (API + file downloads), markdown writer (mrkdwn conversion + file output), and config (env loading). Tests mock all Slack API calls.

**Tech Stack:** Python 3.10+, slack_sdk, requests, pytest

**Design doc:** `docs/plans/2026-02-24-slack-to-markdown-design.md`

---

### Task 1: Project Scaffolding

**Files:**
- Create: `slack-to-markdown/requirements.txt`
- Create: `slack-to-markdown/.env.example`
- Create: `slack-to-markdown/src/__init__.py`
- Create: `slack-to-markdown/tests/__init__.py`

**Step 1: Create directory structure and dependencies**

```bash
mkdir -p slack-to-markdown/src slack-to-markdown/tests
```

Write `slack-to-markdown/requirements.txt`:
```
slack_sdk>=3.27.0
requests>=2.31.0
python-dotenv>=1.0.0
pytest>=7.0.0
```

Write `slack-to-markdown/.env.example`:
```
SLACK_USER_TOKEN=xoxp-your-token-here
```

Write empty `slack-to-markdown/src/__init__.py` and `slack-to-markdown/tests/__init__.py`.

**Step 2: Create virtualenv and install deps**

Run:
```bash
cd slack-to-markdown && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
```
Expected: All packages install successfully.

**Step 3: Verify pytest runs**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/ -v`
Expected: "no tests ran" (0 collected), exit code 5 — that's fine.

**Step 4: Commit**

```bash
git add slack-to-markdown/requirements.txt slack-to-markdown/.env.example slack-to-markdown/src/__init__.py slack-to-markdown/tests/__init__.py
git commit -m "feat(slack-to-markdown): scaffold project structure"
```

---

### Task 2: URL Parser

**Files:**
- Create: `slack-to-markdown/tests/test_url_parser.py`
- Create: `slack-to-markdown/src/url_parser.py`

**Step 1: Write the failing tests**

Write `slack-to-markdown/tests/test_url_parser.py`:
```python
import pytest
from src.url_parser import parse_slack_url


def test_parse_standard_thread_url():
    """Parse a standard Slack thread URL into channel_id and thread_ts."""
    url = "https://myworkspace.slack.com/archives/C06ABCDEF/p1708789200123456"
    result = parse_slack_url(url)
    assert result.channel_id == "C06ABCDEF"
    assert result.thread_ts == "1708789200.123456"


def test_parse_url_with_query_params():
    """Slack URLs sometimes have query params like ?thread_ts=... — ignore them."""
    url = "https://myworkspace.slack.com/archives/C06ABCDEF/p1708789200123456?thread_ts=1708789200.123456&cid=C06ABCDEF"
    result = parse_slack_url(url)
    assert result.channel_id == "C06ABCDEF"
    assert result.thread_ts == "1708789200.123456"


def test_parse_url_different_channel_prefixes():
    """Channel IDs can start with C (channel), G (group), D (DM)."""
    for prefix in ["C", "G", "D"]:
        url = f"https://work.slack.com/archives/{prefix}12345678/p1000000000000000"
        result = parse_slack_url(url)
        assert result.channel_id == f"{prefix}12345678"
        assert result.thread_ts == "1000000000.000000"


def test_parse_invalid_url_raises():
    """Non-Slack URLs should raise ValueError."""
    with pytest.raises(ValueError, match="Invalid Slack thread URL"):
        parse_slack_url("https://google.com/search?q=test")


def test_parse_url_missing_timestamp_raises():
    """URL with channel but no message timestamp should raise ValueError."""
    with pytest.raises(ValueError, match="Invalid Slack thread URL"):
        parse_slack_url("https://myworkspace.slack.com/archives/C06ABCDEF")


def test_parse_url_empty_string_raises():
    """Empty string should raise ValueError."""
    with pytest.raises(ValueError, match="Invalid Slack thread URL"):
        parse_slack_url("")
```

**Step 2: Run tests to verify they fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_url_parser.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'src.url_parser'`

**Step 3: Write the implementation**

Write `slack-to-markdown/src/url_parser.py`:
```python
import re
from dataclasses import dataclass
from urllib.parse import urlparse


@dataclass
class SlackThreadRef:
    channel_id: str
    thread_ts: str


def parse_slack_url(url: str) -> SlackThreadRef:
    """Parse a Slack thread URL into channel_id and thread_ts.

    URL format: https://workspace.slack.com/archives/CHANNEL_ID/pTIMESTAMP
    Timestamp conversion: strip 'p', insert '.' before last 6 digits.
    """
    parsed = urlparse(url)
    match = re.match(
        r"^/archives/([A-Z][A-Z0-9]+)/p(\d{16})$",
        parsed.path,
    )
    if not match or not parsed.hostname or not parsed.hostname.endswith(".slack.com"):
        raise ValueError(
            "Invalid Slack thread URL. "
            "Expected format: https://workspace.slack.com/archives/CHANNEL_ID/pTIMESTAMP"
        )

    channel_id = match.group(1)
    raw_ts = match.group(2)
    thread_ts = f"{raw_ts[:10]}.{raw_ts[10:]}"

    return SlackThreadRef(channel_id=channel_id, thread_ts=thread_ts)
```

**Step 4: Run tests to verify they pass**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_url_parser.py -v`
Expected: All 6 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/url_parser.py slack-to-markdown/tests/test_url_parser.py
git commit -m "feat(slack-to-markdown): add URL parser with tests"
```

---

### Task 3: Config Module

**Files:**
- Create: `slack-to-markdown/tests/test_config.py`
- Create: `slack-to-markdown/src/config.py`

**Step 1: Write the failing tests**

Write `slack-to-markdown/tests/test_config.py`:
```python
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
```

**Step 2: Run tests to verify they fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_config.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'src.config'`

**Step 3: Write the implementation**

Write `slack-to-markdown/src/config.py`:
```python
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
```

**Step 4: Run tests to verify they pass**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_config.py -v`
Expected: All 3 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/config.py slack-to-markdown/tests/test_config.py
git commit -m "feat(slack-to-markdown): add config module with .env loading"
```

---

### Task 4: Slack Client — Fetch Thread & Resolve Users

**Files:**
- Create: `slack-to-markdown/tests/test_slack_client.py`
- Create: `slack-to-markdown/src/slack_client.py`

**Step 1: Write the failing tests**

Write `slack-to-markdown/tests/test_slack_client.py`:
```python
import os
import tempfile
from unittest.mock import MagicMock, patch, call
from src.slack_client import SlackThread, fetch_thread


def _make_message(user="U123", text="hello", ts="1700000000.000001", files=None, bot_id=None):
    """Helper to build a Slack message dict."""
    msg = {"type": "message", "text": text, "ts": ts}
    if bot_id:
        msg["bot_id"] = bot_id
    else:
        msg["user"] = user
    if files:
        msg["files"] = files
    return msg


def _mock_client(replies_pages=None, channel_info=None, users=None, bots=None):
    """Build a mock WebClient with configured responses."""
    client = MagicMock()

    # conversations_info
    if channel_info is None:
        channel_info = {"channel": {"name": "general", "id": "C123"}}
    client.conversations_info.return_value = channel_info

    # conversations_replies — support pagination
    if replies_pages is None:
        replies_pages = [
            {
                "messages": [_make_message(ts="1700000000.000000", text="parent"), _make_message(ts="1700000000.000001", text="reply 1")],
                "has_more": False,
                "response_metadata": {"next_cursor": ""},
            }
        ]
    client.conversations_replies.side_effect = replies_pages

    # users_info
    if users is None:
        users = {"U123": {"user": {"profile": {"display_name": "Alice", "real_name": "Alice Smith"}}}}
    def users_info_side_effect(user=None):
        return users.get(user, {"user": {"profile": {"display_name": "", "real_name": user}}})
    client.users_info.side_effect = users_info_side_effect

    # bots_info
    if bots is None:
        bots = {}
    def bots_info_side_effect(bot=None):
        if bot in bots:
            return bots[bot]
        raise Exception("bot_not_found")
    client.bots_info.side_effect = bots_info_side_effect

    return client


def test_fetch_thread_basic():
    """Fetch a simple thread with parent + 1 reply."""
    client = _mock_client()
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert thread.channel_name == "general"
    assert len(thread.messages) == 2
    assert thread.messages[0]["text"] == "parent"
    assert thread.messages[1]["text"] == "reply 1"


def test_fetch_thread_resolves_user_names():
    """User IDs are resolved to display names."""
    client = _mock_client()
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert thread.user_names["U123"] == "Alice"


def test_fetch_thread_user_display_name_fallback_to_real_name():
    """When display_name is empty, fall back to real_name."""
    users = {"U456": {"user": {"profile": {"display_name": "", "real_name": "Bob Jones"}}}}
    replies = [
        {
            "messages": [_make_message(user="U456", ts="1700000000.000000")],
            "has_more": False,
            "response_metadata": {"next_cursor": ""},
        }
    ]
    client = _mock_client(replies_pages=replies, users=users)
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert thread.user_names["U456"] == "Bob Jones"


def test_fetch_thread_resolves_bot_names():
    """Bot messages resolved via bots.info."""
    replies = [
        {
            "messages": [_make_message(bot_id="B789", text="bot says hi", ts="1700000000.000000")],
            "has_more": False,
            "response_metadata": {"next_cursor": ""},
        }
    ]
    bots = {"B789": {"bot": {"name": "DeployBot"}}}
    client = _mock_client(replies_pages=replies, bots=bots)
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert thread.user_names["B789"] == "DeployBot"


def test_fetch_thread_bot_fallback_to_username():
    """When bots.info fails, fall back to username field."""
    msg = _make_message(bot_id="BBAD", text="bot msg", ts="1700000000.000000")
    msg["username"] = "fallback-bot"
    replies = [
        {
            "messages": [msg],
            "has_more": False,
            "response_metadata": {"next_cursor": ""},
        }
    ]
    client = _mock_client(replies_pages=replies)
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert thread.user_names["BBAD"] == "fallback-bot"


def test_fetch_thread_pagination():
    """Thread with >1 page of replies uses cursor pagination."""
    page1 = {
        "messages": [_make_message(ts="1700000000.000000", text="parent")],
        "has_more": True,
        "response_metadata": {"next_cursor": "cursor_abc"},
    }
    page2 = {
        "messages": [_make_message(ts="1700000000.000001", text="reply on page 2")],
        "has_more": False,
        "response_metadata": {"next_cursor": ""},
    }
    client = _mock_client(replies_pages=[page1, page2])
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert len(thread.messages) == 2
    assert thread.messages[1]["text"] == "reply on page 2"
    # Verify cursor was passed on second call
    assert client.conversations_replies.call_count == 2


def test_fetch_thread_caches_user_lookups():
    """Same user ID across messages should only call users_info once."""
    replies = [
        {
            "messages": [
                _make_message(user="U123", ts="1700000000.000000", text="msg1"),
                _make_message(user="U123", ts="1700000000.000001", text="msg2"),
                _make_message(user="U123", ts="1700000000.000002", text="msg3"),
            ],
            "has_more": False,
            "response_metadata": {"next_cursor": ""},
        }
    ]
    client = _mock_client(replies_pages=replies)
    thread = fetch_thread(client, "C123", "1700000000.000000")

    assert client.users_info.call_count == 1
```

**Step 2: Run tests to verify they fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_slack_client.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'src.slack_client'`

**Step 3: Write the implementation**

Write `slack-to-markdown/src/slack_client.py`:
```python
from dataclasses import dataclass, field
from slack_sdk import WebClient


@dataclass
class SlackThread:
    channel_name: str
    channel_id: str
    thread_ts: str
    messages: list[dict]
    user_names: dict[str, str] = field(default_factory=dict)


def fetch_thread(client: WebClient, channel_id: str, thread_ts: str) -> SlackThread:
    """Fetch a complete Slack thread with resolved user/bot names."""

    # 1. Get channel info
    channel_resp = client.conversations_info(channel=channel_id)
    channel_name = channel_resp["channel"]["name"]

    # 2. Fetch all replies with cursor pagination
    messages = []
    cursor = None
    while True:
        kwargs = {"channel": channel_id, "ts": thread_ts, "limit": 200}
        if cursor:
            kwargs["cursor"] = cursor
        resp = client.conversations_replies(**kwargs)
        messages.extend(resp["messages"])
        if not resp.get("has_more"):
            break
        cursor = resp.get("response_metadata", {}).get("next_cursor")
        if not cursor:
            break

    # 3. Resolve unique user/bot IDs to display names
    user_names = {}
    for msg in messages:
        user_id = msg.get("user")
        bot_id = msg.get("bot_id")

        if user_id and user_id not in user_names:
            user_names[user_id] = _resolve_user(client, user_id)
        elif bot_id and bot_id not in user_names:
            user_names[bot_id] = _resolve_bot(client, bot_id, msg)

    return SlackThread(
        channel_name=channel_name,
        channel_id=channel_id,
        thread_ts=thread_ts,
        messages=messages,
        user_names=user_names,
    )


def _resolve_user(client: WebClient, user_id: str) -> str:
    """Resolve a user ID to display name, falling back to real_name then raw ID."""
    try:
        resp = client.users_info(user=user_id)
        profile = resp["user"]["profile"]
        return profile.get("display_name") or profile.get("real_name") or user_id
    except Exception:
        return user_id


def _resolve_bot(client: WebClient, bot_id: str, msg: dict) -> str:
    """Resolve a bot ID to name, falling back to message username then raw ID."""
    try:
        resp = client.bots_info(bot=bot_id)
        return resp["bot"]["name"]
    except Exception:
        return msg.get("username") or bot_id
```

**Step 4: Run tests to verify they pass**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_slack_client.py -v`
Expected: All 8 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/slack_client.py slack-to-markdown/tests/test_slack_client.py
git commit -m "feat(slack-to-markdown): add Slack client with thread fetch and user resolution"
```

---

### Task 5: Slack Client — File Downloads

**Files:**
- Modify: `slack-to-markdown/tests/test_slack_client.py`
- Modify: `slack-to-markdown/src/slack_client.py`

**Step 1: Write the failing tests**

Append to `slack-to-markdown/tests/test_slack_client.py`:
```python
from src.slack_client import download_files


def test_download_files_saves_images(tmp_path):
    """Files from messages are downloaded to the attachments directory."""
    messages = [
        _make_message(
            ts="1700000000.000000",
            files=[{"name": "screenshot.png", "url_private_download": "https://files.slack.com/files-pri/T123/screenshot.png", "mimetype": "image/png"}],
        ),
    ]
    with patch("src.slack_client.requests") as mock_requests:
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.content = b"fake-png-data"
        mock_resp.raise_for_status = MagicMock()
        mock_requests.get.return_value = mock_resp

        file_map = download_files(messages, str(tmp_path), "xoxp-test")

    saved_file = tmp_path / "screenshot.png"
    assert saved_file.exists()
    assert saved_file.read_bytes() == b"fake-png-data"
    assert file_map["1700000000.000000"][0]["local_path"] == "screenshot.png"


def test_download_files_handles_duplicate_names(tmp_path):
    """Two files with the same name get deduplicated."""
    messages = [
        _make_message(
            ts="1700000000.000000",
            files=[
                {"name": "image.png", "url_private_download": "https://files.slack.com/1", "mimetype": "image/png"},
                {"name": "image.png", "url_private_download": "https://files.slack.com/2", "mimetype": "image/png"},
            ],
        ),
    ]
    with patch("src.slack_client.requests") as mock_requests:
        mock_resp = MagicMock()
        mock_resp.status_code = 200
        mock_resp.content = b"data"
        mock_resp.raise_for_status = MagicMock()
        mock_requests.get.return_value = mock_resp

        file_map = download_files(messages, str(tmp_path), "xoxp-test")

    assert (tmp_path / "image.png").exists()
    assert (tmp_path / "image_1.png").exists()


def test_download_files_failed_download_returns_placeholder(tmp_path):
    """Failed download records error but doesn't crash."""
    messages = [
        _make_message(
            ts="1700000000.000000",
            files=[{"name": "broken.pdf", "url_private_download": "https://files.slack.com/broken", "mimetype": "application/pdf"}],
        ),
    ]
    with patch("src.slack_client.requests") as mock_requests:
        mock_requests.get.side_effect = Exception("network error")

        file_map = download_files(messages, str(tmp_path), "xoxp-test")

    assert file_map["1700000000.000000"][0]["error"] == "network error"


def test_download_files_no_files_returns_empty(tmp_path):
    """Messages with no files produce empty file_map."""
    messages = [_make_message(ts="1700000000.000000")]

    file_map = download_files(messages, str(tmp_path), "xoxp-test")
    assert file_map == {}
```

**Step 2: Run tests to verify new tests fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_slack_client.py::test_download_files_saves_images -v`
Expected: FAIL — `ImportError: cannot import name 'download_files'`

**Step 3: Write the implementation**

Add to `slack-to-markdown/src/slack_client.py` (add `import requests` at the top, then this function at the bottom):

```python
import requests  # add at top of file with other imports

def download_files(messages: list[dict], attachments_dir: str, token: str) -> dict[str, list[dict]]:
    """Download all files from messages into attachments_dir.

    Returns a map of message_ts -> list of file info dicts with keys:
      - name: original filename
      - local_path: filename saved to disk (relative to attachments_dir)
      - mimetype: MIME type
      - error: set if download failed
    """
    file_map: dict[str, list[dict]] = {}
    used_names: set[str] = set()

    for msg in messages:
        msg_files = msg.get("files")
        if not msg_files:
            continue

        ts = msg["ts"]
        file_map[ts] = []

        for f in msg_files:
            name = f.get("name", "unknown")
            url = f.get("url_private_download")
            mimetype = f.get("mimetype", "application/octet-stream")

            # Deduplicate filenames
            local_name = _unique_filename(name, used_names)
            used_names.add(local_name)

            info = {"name": name, "local_path": local_name, "mimetype": mimetype}

            if not url:
                info["error"] = "no download URL"
                file_map[ts].append(info)
                continue

            try:
                resp = requests.get(url, headers={"Authorization": f"Bearer {token}"})
                resp.raise_for_status()
                filepath = os.path.join(attachments_dir, local_name)
                with open(filepath, "wb") as fh:
                    fh.write(resp.content)
            except Exception as e:
                info["error"] = str(e)

            file_map[ts].append(info)

    return file_map


def _unique_filename(name: str, used: set[str]) -> str:
    """Return a unique filename, appending _N if name already used."""
    if name not in used:
        return name
    base, ext = os.path.splitext(name)
    counter = 1
    while f"{base}_{counter}{ext}" in used:
        counter += 1
    return f"{base}_{counter}{ext}"
```

Also add `import os` at the top of the file.

**Step 4: Run all slack_client tests**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_slack_client.py -v`
Expected: All 12 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/slack_client.py slack-to-markdown/tests/test_slack_client.py
git commit -m "feat(slack-to-markdown): add file download with dedup and error handling"
```

---

### Task 6: Markdown Writer — mrkdwn Conversion

**Files:**
- Create: `slack-to-markdown/tests/test_markdown_writer.py`
- Create: `slack-to-markdown/src/markdown_writer.py`

**Step 1: Write the failing tests**

Write `slack-to-markdown/tests/test_markdown_writer.py`:
```python
from src.markdown_writer import convert_mrkdwn


def test_convert_bold():
    assert convert_mrkdwn("this is *bold* text", {}) == "this is **bold** text"


def test_convert_italic():
    assert convert_mrkdwn("this is _italic_ text", {}) == "this is *italic* text"


def test_convert_strikethrough():
    assert convert_mrkdwn("this is ~struck~ text", {}) == "this is ~~struck~~ text"


def test_convert_user_mention():
    users = {"U123": "Alice"}
    assert convert_mrkdwn("hey <@U123> check this", users) == "hey **@Alice** check this"


def test_convert_user_mention_unknown():
    assert convert_mrkdwn("hey <@U999> check this", {}) == "hey **@U999** check this"


def test_convert_channel_link():
    assert convert_mrkdwn("see <#C123|general>", {}) == "see #general"


def test_convert_url_with_label():
    assert convert_mrkdwn("check <https://example.com|this link>", {}) == "check [this link](https://example.com)"


def test_convert_url_without_label():
    assert convert_mrkdwn("see <https://example.com>", {}) == "see https://example.com"


def test_convert_code_blocks_untouched():
    """Code inside backticks should not be converted."""
    text = "run `*this command*` now"
    result = convert_mrkdwn(text, {})
    assert "`*this command*`" in result


def test_convert_triple_backtick_blocks_untouched():
    text = "```\n*bold in code*\n_italic in code_\n```"
    result = convert_mrkdwn(text, {})
    assert "*bold in code*" in result
    assert "_italic in code_" in result


def test_convert_mixed_formatting():
    users = {"U123": "Alice"}
    text = "*important:* hey <@U123>, check <https://example.com|this> — it's _urgent_ ~maybe~"
    result = convert_mrkdwn(text, users)
    assert "**important:**" in result
    assert "**@Alice**" in result
    assert "[this](https://example.com)" in result
    assert "*urgent*" in result
    assert "~~maybe~~" in result
```

**Step 2: Run tests to verify they fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_markdown_writer.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'src.markdown_writer'`

**Step 3: Write the implementation**

Write `slack-to-markdown/src/markdown_writer.py`:
```python
import re


def convert_mrkdwn(text: str, user_names: dict[str, str]) -> str:
    """Convert Slack mrkdwn to standard Markdown.

    Handles: bold, italic, strikethrough, user mentions, channel links, URLs.
    Preserves code blocks and inline code.
    """
    # Extract and preserve code blocks/inline code
    preserved = []

    def _preserve(match):
        preserved.append(match.group(0))
        return f"\x00CODE{len(preserved) - 1}\x00"

    # Preserve triple-backtick blocks first, then inline code
    text = re.sub(r"```[\s\S]*?```", _preserve, text)
    text = re.sub(r"`[^`]+`", _preserve, text)

    # User mentions: <@U123> → **@DisplayName**
    def _replace_user(match):
        uid = match.group(1)
        name = user_names.get(uid, uid)
        return f"**@{name}**"

    text = re.sub(r"<@(\w+)>", _replace_user, text)

    # Channel links: <#C123|channel-name> → #channel-name
    text = re.sub(r"<#\w+\|([^>]+)>", r"#\1", text)

    # URLs with labels: <https://...|label> → [label](url)
    text = re.sub(r"<(https?://[^|>]+)\|([^>]+)>", r"[\2](\1)", text)

    # URLs without labels: <https://...> → url
    text = re.sub(r"<(https?://[^>]+)>", r"\1", text)

    # Bold: *text* → **text** (but not inside words or at code boundaries)
    text = re.sub(r"(?<![\\`\w])\*([^\s*](?:[^*]*[^\s*])?)\*(?![\\`\w])", r"**\1**", text)

    # Italic: _text_ → *text*
    text = re.sub(r"(?<![\\`\w])_([^\s_](?:[^_]*[^\s_])?)_(?![\\`\w])", r"*\1*", text)

    # Strikethrough: ~text~ → ~~text~~
    text = re.sub(r"(?<![\\`\w])~([^\s~](?:[^~]*[^\s~])?)~(?![\\`\w])", r"~~\1~~", text)

    # Restore preserved code blocks
    for i, code in enumerate(preserved):
        text = text.replace(f"\x00CODE{i}\x00", code)

    return text
```

**Step 4: Run tests to verify they pass**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_markdown_writer.py -v`
Expected: All 12 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/markdown_writer.py slack-to-markdown/tests/test_markdown_writer.py
git commit -m "feat(slack-to-markdown): add Slack mrkdwn to markdown converter"
```

---

### Task 7: Markdown Writer — Document Generation

**Files:**
- Modify: `slack-to-markdown/tests/test_markdown_writer.py`
- Modify: `slack-to-markdown/src/markdown_writer.py`

**Step 1: Write the failing tests**

Append to `slack-to-markdown/tests/test_markdown_writer.py`:
```python
import os
import tempfile
from src.markdown_writer import generate_markdown
from src.slack_client import SlackThread


def _thread(messages=None, user_names=None, channel_name="general"):
    if messages is None:
        messages = [
            {"user": "U1", "text": "parent message", "ts": "1700000000.000000"},
            {"user": "U2", "text": "reply here", "ts": "1700000060.000000"},
        ]
    if user_names is None:
        user_names = {"U1": "Alice", "U2": "Bob"}
    return SlackThread(
        channel_name=channel_name,
        channel_id="C123",
        thread_ts="1700000000.000000",
        messages=messages,
        user_names=user_names,
    )


def test_generate_markdown_header():
    """Output starts with channel name, date, link, participants."""
    md = generate_markdown(_thread(), {}, "https://workspace.slack.com/archives/C123/p1700000000000000")
    assert "# Slack Thread: #general" in md
    assert "**Thread link:**" in md
    assert "**Participants:** Alice, Bob" in md


def test_generate_markdown_message_format():
    """Each message has ## User — Time header and text."""
    md = generate_markdown(_thread(), {}, "https://example.com")
    assert "## Alice" in md
    assert "parent message" in md
    assert "## Bob" in md
    assert "reply here" in md


def test_generate_markdown_with_image():
    """Image files render as ![name](attachments/name)."""
    file_map = {
        "1700000000.000000": [{"name": "screenshot.png", "local_path": "screenshot.png", "mimetype": "image/png"}]
    }
    thread = _thread(messages=[
        {"user": "U1", "text": "see this", "ts": "1700000000.000000"},
    ], user_names={"U1": "Alice"})
    md = generate_markdown(thread, file_map, "https://example.com", attachments_dirname="my-attachments")
    assert "![screenshot.png](my-attachments/screenshot.png)" in md


def test_generate_markdown_with_non_image_file():
    """Non-image files render as [name](attachments/name)."""
    file_map = {
        "1700000000.000000": [{"name": "report.pdf", "local_path": "report.pdf", "mimetype": "application/pdf"}]
    }
    thread = _thread(messages=[
        {"user": "U1", "text": "attached", "ts": "1700000000.000000"},
    ], user_names={"U1": "Alice"})
    md = generate_markdown(thread, file_map, "https://example.com", attachments_dirname="att")
    assert "[report.pdf](att/report.pdf)" in md
    assert "![" not in md  # not an image embed


def test_generate_markdown_failed_download():
    """Failed downloads show placeholder."""
    file_map = {
        "1700000000.000000": [{"name": "broken.zip", "local_path": "broken.zip", "mimetype": "application/zip", "error": "network error"}]
    }
    thread = _thread(messages=[
        {"user": "U1", "text": "here", "ts": "1700000000.000000"},
    ], user_names={"U1": "Alice"})
    md = generate_markdown(thread, file_map, "https://example.com")
    assert "[Failed to download: broken.zip]" in md


def test_generate_markdown_bot_message():
    """Bot messages display bot name."""
    thread = _thread(
        messages=[{"bot_id": "B1", "text": "deploy complete", "ts": "1700000000.000000"}],
        user_names={"B1": "DeployBot"},
    )
    md = generate_markdown(thread, {}, "https://example.com")
    assert "## DeployBot" in md
```

**Step 2: Run tests to verify new tests fail**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_markdown_writer.py::test_generate_markdown_header -v`
Expected: FAIL — `ImportError: cannot import name 'generate_markdown'`

**Step 3: Write the implementation**

Append to `slack-to-markdown/src/markdown_writer.py`:
```python
from datetime import datetime, timezone


def generate_markdown(
    thread,
    file_map: dict[str, list[dict]],
    original_url: str,
    attachments_dirname: str = "attachments",
) -> str:
    """Generate a complete markdown document from a SlackThread and downloaded files."""
    lines = []

    # Header
    date_ts = float(thread.thread_ts)
    date_str = datetime.fromtimestamp(date_ts, tz=timezone.utc).strftime("%Y-%m-%d")

    # Collect unique participant names in message order
    seen = set()
    participants = []
    for msg in thread.messages:
        uid = msg.get("user") or msg.get("bot_id", "")
        name = thread.user_names.get(uid, uid)
        if name not in seen:
            seen.add(name)
            participants.append(name)

    lines.append(f"# Slack Thread: #{thread.channel_name}")
    lines.append("")
    lines.append(f"**Date:** {date_str}")
    lines.append(f"**Thread link:** {original_url}")
    lines.append(f"**Participants:** {', '.join(participants)}")
    lines.append("")

    # Messages
    for msg in thread.messages:
        lines.append("---")
        lines.append("")

        uid = msg.get("user") or msg.get("bot_id", "")
        name = thread.user_names.get(uid, uid)
        msg_time = datetime.fromtimestamp(float(msg["ts"]), tz=timezone.utc).strftime("%-I:%M %p")
        lines.append(f"## {name} — {msg_time}")
        lines.append("")

        # Message text
        text = msg.get("text", "")
        if text:
            lines.append(convert_mrkdwn(text, thread.user_names))
            lines.append("")

        # Files
        msg_files = file_map.get(msg["ts"], [])
        for f in msg_files:
            if "error" in f:
                lines.append(f"[Failed to download: {f['name']}]")
            elif f["mimetype"].startswith("image/"):
                lines.append(f"![{f['name']}]({attachments_dirname}/{f['local_path']})")
            else:
                lines.append(f"[{f['name']}]({attachments_dirname}/{f['local_path']})")
            lines.append("")

    lines.append("---")
    lines.append("")

    return "\n".join(lines)
```

**Step 4: Run all markdown_writer tests**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/test_markdown_writer.py -v`
Expected: All 18 tests PASS.

**Step 5: Commit**

```bash
git add slack-to-markdown/src/markdown_writer.py slack-to-markdown/tests/test_markdown_writer.py
git commit -m "feat(slack-to-markdown): add markdown document generation"
```

---

### Task 8: CLI Entry Point

**Files:**
- Create: `slack-to-markdown/src/__main__.py`

**Step 1: Write the CLI**

Write `slack-to-markdown/src/__main__.py`:
```python
import argparse
import os
import sys
from datetime import datetime, timezone

from slack_sdk import WebClient

from .config import load_config
from .url_parser import parse_slack_url
from .slack_client import fetch_thread, download_files
from .markdown_writer import generate_markdown


def main():
    parser = argparse.ArgumentParser(
        description="Convert a Slack thread to a markdown file with attachments."
    )
    parser.add_argument("url", help="Slack thread URL")
    parser.add_argument(
        "-o", "--output",
        help="Output directory (default: current directory)",
        default=".",
    )
    args = parser.parse_args()

    # Parse URL
    try:
        ref = parse_slack_url(args.url)
    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

    # Load config
    config = load_config()
    client = WebClient(token=config.token)

    # Fetch thread
    print(f"Fetching thread from channel {ref.channel_id}...")
    thread = fetch_thread(client, ref.channel_id, ref.thread_ts)
    print(f"Found {len(thread.messages)} messages in #{thread.channel_name}")

    # Build output paths
    date_ts = float(thread.thread_ts)
    date_str = datetime.fromtimestamp(date_ts, tz=timezone.utc).strftime("%Y-%m-%d")
    base_name = f"{thread.channel_name}-{date_str}-thread"
    output_dir = os.path.abspath(args.output)
    md_path = os.path.join(output_dir, f"{base_name}.md")
    attachments_dir = os.path.join(output_dir, f"{base_name}-attachments")
    attachments_dirname = f"{base_name}-attachments"

    # Download files
    has_files = any(msg.get("files") for msg in thread.messages)
    file_map = {}
    if has_files:
        os.makedirs(attachments_dir, exist_ok=True)
        print("Downloading attachments...")
        file_map = download_files(thread.messages, attachments_dir, config.token)
        total = sum(len(v) for v in file_map.values())
        errors = sum(1 for v in file_map.values() for f in v if "error" in f)
        print(f"Downloaded {total - errors}/{total} files")

    # Generate markdown
    os.makedirs(output_dir, exist_ok=True)
    md_content = generate_markdown(thread, file_map, args.url, attachments_dirname)
    with open(md_path, "w") as f:
        f.write(md_content)

    print(f"\nSaved: {md_path}")
    if has_files:
        print(f"Attachments: {attachments_dir}")


if __name__ == "__main__":
    main()
```

**Step 2: Verify it runs (help text)**

Run: `cd slack-to-markdown && .venv/bin/python -m src --help`
Expected: Shows argparse help with `url` positional arg and `-o` option.

**Step 3: Verify error handling for bad URL**

Run: `cd slack-to-markdown && .venv/bin/python -m src "https://google.com" 2>&1; echo "exit: $?"`
Expected: "Error: Invalid Slack thread URL..." and exit code 1.

**Step 4: Commit**

```bash
git add slack-to-markdown/src/__main__.py
git commit -m "feat(slack-to-markdown): add CLI entry point"
```

---

### Task 9: Setup Script & CLAUDE.md

**Files:**
- Create: `slack-to-markdown/setup.sh`
- Create: `slack-to-markdown/CLAUDE.md`

**Step 1: Write setup.sh**

Write `slack-to-markdown/setup.sh`:
```bash
#!/bin/bash
# Slack-to-Markdown Setup Script
# Run with: bash setup.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Setting up slack-to-markdown..."
echo ""

# Check Python 3
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version 2>&1 | cut -d' ' -f2)
    echo -e "${GREEN}✓${NC} Python ${PYTHON_VERSION} found"
else
    echo -e "${RED}✗${NC} Python 3 not found. Install Python 3.10+."
    exit 1
fi

# Create virtualenv if needed
VENV_DIR="${SCRIPT_DIR}/.venv"
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment..."
    python3 -m venv "$VENV_DIR"
    echo -e "${GREEN}✓${NC} Virtual environment created"
else
    echo -e "${GREEN}✓${NC} Virtual environment exists"
fi

# Install dependencies
echo "Installing dependencies..."
"${VENV_DIR}/bin/pip" install -q -r "${SCRIPT_DIR}/requirements.txt"
echo -e "${GREEN}✓${NC} Dependencies installed"

# Check .env
if [ -f "${SCRIPT_DIR}/.env" ]; then
    echo -e "${GREEN}✓${NC} .env file found"
else
    echo -e "${YELLOW}○${NC} .env file not found"
    echo "  Copy .env.example to .env and add your Slack user token:"
    echo "    cp ${SCRIPT_DIR}/.env.example ${SCRIPT_DIR}/.env"
    echo "  Then edit .env with your token."
fi

echo ""
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "Usage:"
echo "  cd ${SCRIPT_DIR}"
echo "  .venv/bin/python -m src <slack-thread-url>"
```

**Step 2: Write CLAUDE.md**

Write `slack-to-markdown/CLAUDE.md`:
```markdown
# CLAUDE.md

This file provides guidance to Claude Code when working with code in this project.

## Project Overview

CLI tool to convert a Slack thread URL into a self-contained markdown file with downloaded images/attachments. Designed for feeding Slack conversations into Claude Code.

## Running

```bash
# First-time setup
bash setup.sh

# Convert a Slack thread
.venv/bin/python -m src https://workspace.slack.com/archives/C12345/p1708789200000001

# With custom output directory
.venv/bin/python -m src <url> -o ~/slack-threads/

# Run tests
.venv/bin/python -m pytest tests/ -v
```

## Architecture

```
src/
├── __main__.py        CLI entry point (argparse)
├── url_parser.py      Parse Slack URLs → channel_id + thread_ts
├── slack_client.py    Fetch thread, resolve users/bots, download files
├── markdown_writer.py Slack mrkdwn → Markdown conversion + document generation
└── config.py          Load .env, validate SLACK_USER_TOKEN
```

**Data flow:** URL parsed → conversations.info (channel name) → conversations.replies (all messages, paginated) → users.info/bots.info (resolve names) → download files → generate markdown → write to disk.

## Key Conventions

- All Slack API calls go through `slack_sdk.WebClient` (handles rate limiting)
- File downloads use `requests` with Bearer token auth
- Tests mock all API calls — no real Slack access needed
- Config via `.env` file with `SLACK_USER_TOKEN`

## Dependencies

Python packages: `slack_sdk`, `requests`, `python-dotenv`. See `requirements.txt`.
```

**Step 3: Make setup.sh executable and commit**

```bash
chmod +x slack-to-markdown/setup.sh
git add slack-to-markdown/setup.sh slack-to-markdown/CLAUDE.md
git commit -m "feat(slack-to-markdown): add setup script and CLAUDE.md"
```

---

### Task 10: Add .env to .gitignore & Run Full Test Suite

**Files:**
- Modify: `.gitignore`

**Step 1: Add .env to gitignore**

Append to root `.gitignore`:
```
# Environment files
.env
```

**Step 2: Run the full test suite**

Run: `cd slack-to-markdown && .venv/bin/python -m pytest tests/ -v`
Expected: All tests pass (6 + 3 + 12 + 18 = 39 tests total).

**Step 3: Commit**

```bash
git add .gitignore
git commit -m "chore: add .env to gitignore"
```

---

Plan complete and saved to `docs/plans/2026-02-24-slack-to-markdown-implementation.md`. Two execution options:

**1. Subagent-Driven (this session)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** — Open new session with executing-plans, batch execution with checkpoints

Which approach?