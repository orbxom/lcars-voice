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
