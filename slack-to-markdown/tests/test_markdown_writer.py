import os
import tempfile
from src.markdown_writer import convert_mrkdwn, generate_markdown
from src.slack_client import SlackThread


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
