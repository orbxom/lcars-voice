import re
from datetime import datetime, timezone


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
    text = re.sub(r"(?<![\\`\w*])\*([^\s*](?:[^*]*[^\s*])?)\*(?![\\`\w*])", r"**\1**", text)

    # Italic: _text_ → *text*
    text = re.sub(r"(?<![\\`\w])_([^\s_](?:[^_]*[^\s_])?)_(?![\\`\w])", r"*\1*", text)

    # Strikethrough: ~text~ → ~~text~~
    text = re.sub(r"(?<![\\`\w])~([^\s~](?:[^~]*[^\s~])?)~(?![\\`\w])", r"~~\1~~", text)

    # Restore preserved code blocks
    for i, code in enumerate(preserved):
        text = text.replace(f"\x00CODE{i}\x00", code)

    return text


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
