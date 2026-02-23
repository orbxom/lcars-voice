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
    text = re.sub(r"(?<![\\`\w*])\*([^\s*](?:[^*]*[^\s*])?)\*(?![\\`\w*])", r"**\1**", text)

    # Italic: _text_ → *text*
    text = re.sub(r"(?<![\\`\w])_([^\s_](?:[^_]*[^\s_])?)_(?![\\`\w])", r"*\1*", text)

    # Strikethrough: ~text~ → ~~text~~
    text = re.sub(r"(?<![\\`\w])~([^\s~](?:[^~]*[^\s~])?)~(?![\\`\w])", r"~~\1~~", text)

    # Restore preserved code blocks
    for i, code in enumerate(preserved):
        text = text.replace(f"\x00CODE{i}\x00", code)

    return text
