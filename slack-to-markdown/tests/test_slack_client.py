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
