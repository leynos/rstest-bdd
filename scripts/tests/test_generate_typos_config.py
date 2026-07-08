"""Unit tests for the ``typos.toml`` generator script.

The generator is imported by name because ``conftest.py`` places the
``scripts`` directory on ``sys.path``. The committed ``typos.toml`` is
compared against the generator output so the two cannot drift apart.
"""

from __future__ import annotations

import pathlib

import generate_typos_config as gen
import pytest
from hypothesis import given
from hypothesis import strategies as st

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]
SAFE_FILENAME_CHARS = tuple("abcdefghijklmnopqrstuvwxyz0123456789_-")
SAFE_TYPOS_FILENAMES = st.lists(
    st.sampled_from(SAFE_FILENAME_CHARS),
    min_size=1,
    max_size=32,
).map(lambda chars: f"{''.join(chars)}.toml")


@pytest.fixture(name="rendered_config", scope="module")
def rendered_config_fixture() -> str:
    """Render the generator output once for property tests."""
    return gen.render_config()


def test_render_config_emits_every_stem_and_suffix_pair() -> None:
    """Every stem inflection gets an -ise correction and an -ize identity."""
    rendered = gen.render_config()
    for stem in gen.STEMS:
        for ise, ize in gen.SUFFIX_PAIRS:
            assert f'{stem}{ise} = "{stem}{ize}"' in rendered
            assert f'{stem}{ize} = "{stem}{ize}"' in rendered


def test_render_config_accepts_extra_words_verbatim() -> None:
    """Every extra accepted word gets an identity entry."""
    rendered = gen.render_config()
    for word in gen.EXTRA_ACCEPTED_WORDS:
        assert f'{word} = "{word}"' in rendered


def test_render_config_ends_with_trailing_newline() -> None:
    """The rendered document ends with exactly one trailing newline."""
    rendered = gen.render_config()
    assert rendered.endswith("\n")
    assert not rendered.endswith("\n\n")


@given(data=st.data())
def test_render_config_property_emits_sampled_stem_suffix_pair(
    rendered_config: str,
    data: st.DataObject,
) -> None:
    """A sampled stem and suffix pair gets correction and identity entries."""
    stem = data.draw(st.sampled_from(gen.STEMS))
    ise, ize = data.draw(st.sampled_from(gen.SUFFIX_PAIRS))

    assert f'{stem}{ise} = "{stem}{ize}"' in rendered_config
    assert f'{stem}{ize} = "{stem}{ize}"' in rendered_config


@given(data=st.data())
def test_render_config_property_accepts_sampled_extra_word(
    rendered_config: str,
    data: st.DataObject,
) -> None:
    """A sampled extra accepted word gets an identity entry."""
    word = data.draw(st.sampled_from(sorted(gen.EXTRA_ACCEPTED_WORDS)))

    assert f'{word} = "{word}"' in rendered_config


@given(filename=SAFE_TYPOS_FILENAMES)
def test_main_property_writes_rendered_config_without_mutation(
    filename: str,
    tmp_path_factory: pytest.TempPathFactory,
) -> None:
    """main() writes rendered content exactly to a sampled safe filename."""
    output = tmp_path_factory.mktemp("typos") / filename
    gen.main(output)

    assert output.read_text(encoding="utf-8") == gen.render_config()


def test_main_writes_rendered_config_to_explicit_path(
    tmp_path: pathlib.Path,
) -> None:
    """main() writes the rendered configuration to the given output path."""
    output = tmp_path / "typos.toml"
    gen.main(output)
    assert output.read_text(encoding="utf-8") == gen.render_config()


def test_main_default_path_resolves_to_repository_root(
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """main() defaults to typos.toml two levels above the script file."""
    fake_script = tmp_path / "scripts" / "generate_typos_config.py"
    monkeypatch.setattr(gen, "__file__", str(fake_script))
    gen.main()
    written = tmp_path / "typos.toml"
    assert written.read_text(encoding="utf-8") == gen.render_config()


def test_committed_config_matches_generator_output() -> None:
    """The committed typos.toml must not drift from the generator."""
    committed = (REPOSITORY_ROOT / "typos.toml").read_text(encoding="utf-8")
    assert committed == gen.render_config()
