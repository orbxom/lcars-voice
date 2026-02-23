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
