import os
from dataclasses import dataclass, field
from slack_sdk import WebClient
import requests


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
