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
