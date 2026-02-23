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
