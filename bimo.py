#!/usr/bin/env python3
"""
Bimo — a local CLI coding agent powered by any OpenAI-compatible API.

Configuration is managed interactively inside the REPL and persisted to
``~/.config/bimo/config.json``.  Providers and models are auto-detected.

Usage:
    python bimo.py                          # interactive chat (setup wizard on first run)
    python bimo.py --project-dir /path      # set project root
    python bimo.py --yes                    # auto-approve file changes
    python bimo.py --provider NAME          # use a specific saved provider
    python bimo.py --model NAME             # override model for this session
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from difflib import unified_diff
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# 1. Configuration — persistence, providers, model auto-detection
# ---------------------------------------------------------------------------

BIMO_CONFIG_DIR = Path.home() / ".config" / "bimo"
BIMO_CONFIG_FILE = BIMO_CONFIG_DIR / "config.json"

MAX_AGENT_ITERATIONS = 50

# ANSI helpers
BOLD = "\033[1m"
DIM = "\033[2m"
RESET = "\033[0m"
CYAN = "\033[36m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
RED = "\033[31m"
MAGENTA = "\033[35m"
BLUE = "\033[34m"


def _style(text: str, *codes: str) -> str:
    return "".join(codes) + text + RESET


# ---- config load / save ---------------------------------------------------


def _default_config() -> dict[str, Any]:
    return {
        "providers": {},
        "active_provider": None,
        "auto_approve": False,
        "max_iterations": MAX_AGENT_ITERATIONS,
        "project_dir": None,
    }


def load_config() -> dict[str, Any]:
    if BIMO_CONFIG_FILE.exists():
        try:
            return json.loads(BIMO_CONFIG_FILE.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            pass
    return _default_config()


def save_config(cfg: dict[str, Any]) -> None:
    BIMO_CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    BIMO_CONFIG_FILE.write_text(
        json.dumps(cfg, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


# ---- OpenAI client factory ------------------------------------------------


def _make_client(provider: dict[str, Any]) -> Any:
    """Create an OpenAI client from a provider dict."""
    try:
        from openai import OpenAI
    except ImportError:
        print(_style("  Error: 'openai' package not installed.", RED))
        print("  Install with:  pip install openai")
        print("  Or:            uv pip install openai")
        sys.exit(1)
    return OpenAI(api_key=provider["api_key"], base_url=provider["base_url"])


# ---- model auto-detection -------------------------------------------------


def fetch_models(provider: dict[str, Any]) -> list[str]:
    """Fetch available models from a provider's /v1/models endpoint.

    Returns a sorted list of model id strings, or an empty list on failure.
    """
    try:
        client = _make_client(provider)
        response = client.models.list()
        models = sorted(m.id for m in response.data)
        return models
    except Exception:
        return []


# ---- interactive helpers ---------------------------------------------------


def _input(prompt: str, default: str = "") -> str:
    suffix = f" [{default}]" if default else ""
    try:
        val = input(f"{prompt}{suffix}: ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        return default
    return val if val else default


def _choose_from_list(items: list[str], prompt: str = "  Pick one") -> str | None:
    """Display a numbered list and let the user choose.  Returns the item or None."""
    for i, item in enumerate(items, 1):
        print(f"    {_style(str(i), CYAN)}  {item}")
    print()
    raw = _input(f"{prompt} (number or name)", "1")
    if not raw:
        return None
    # by number
    if raw.isdigit():
        idx = int(raw) - 1
        if 0 <= idx < len(items):
            return items[idx]
    # by name (substring match)
    for item in items:
        if raw.lower() in item.lower():
            return item
    return raw  # return as-is, let caller handle


# ---- provider setup wizard -------------------------------------------------


PROVIDER_PRESETS: dict[str, dict[str, str]] = {
    "OpenAI": {"base_url": "https://api.openai.com/v1", "key_hint": "sk-..."},
    "OpenRouter": {"base_url": "https://openrouter.ai/api/v1", "key_hint": "sk-or-..."},
    "Ollama (local)": {"base_url": "http://localhost:11434/v1", "key_hint": "ollama"},
    "LM Studio (local)": {
        "base_url": "http://localhost:1234/v1",
        "key_hint": "lm-studio",
    },
    "vLLM (local)": {"base_url": "http://localhost:8000/v1", "key_hint": "vllm"},
    "Custom": {"base_url": "", "key_hint": ""},
}


def _setup_provider_interactive(cfg: dict[str, Any], *, is_first: bool = False) -> bool:
    """Walk the user through adding a provider.  Returns True if a provider was added."""
    print()
    if is_first:
        print(_style("  Let's set up your first provider.", BOLD))
    else:
        print(_style("  Add a new provider.", BOLD))
    print()

    # 1) Name
    name = _input("  Provider name (e.g. 'openai', 'my-ollama')", "")
    if not name:
        print(_style("  Cancelled.", DIM))
        return False
    name = name.strip().lower().replace(" ", "-")

    if name in cfg["providers"]:
        overwrite = _input(f"  Provider '{name}' already exists. Overwrite? [y/N]", "n")
        if overwrite.lower() not in ("y", "yes"):
            print(_style("  Cancelled.", DIM))
            return False

    # 2) Pick a preset or go custom
    print()
    print(_style("  Choose a provider type:", BOLD))
    preset_names = list(PROVIDER_PRESETS.keys())
    for i, pname in enumerate(preset_names, 1):
        info = PROVIDER_PRESETS[pname]
        hint = info["base_url"] or "custom URL"
        print(f"    {_style(str(i), CYAN)}  {_style(pname, BOLD)}  ({hint})")
    print()
    preset_choice = _input("  Pick (number or name)", "1")

    preset: dict[str, str] | None = None
    if preset_choice.isdigit():
        idx = int(preset_choice) - 1
        if 0 <= idx < len(preset_names):
            preset = PROVIDER_PRESETS[preset_names[idx]]
    else:
        for pname in preset_names:
            if preset_choice.lower() in pname.lower():
                preset = PROVIDER_PRESETS[pname]
                break

    # 3) Base URL
    default_url = preset["base_url"] if preset else ""
    base_url = _input("  Base URL", default_url)
    if not base_url:
        print(_style("  A base URL is required. Cancelled.", DIM))
        return False

    # 4) API key
    default_key = preset["key_hint"] if preset else ""
    print()
    print(f"  Enter your API key (for local providers like Ollama, any value works).")
    api_key = _input("  API key", default_key)
    if not api_key:
        print(_style("  An API key is required. Cancelled.", DIM))
        return False

    provider: dict[str, Any] = {
        "base_url": base_url,
        "api_key": api_key,
        "models": [],
        "selected_model": None,
    }

    # 5) Auto-detect models
    print()
    print(_style("  Fetching available models…", DIM))
    models = fetch_models(provider)

    if models:
        print(_style(f"  Found {len(models)} models.", GREEN))
        print()
        print(_style("  Select the default model for this provider:", BOLD))
        chosen = _choose_from_list(models, "  Pick a model")
        if chosen and chosen in models:
            provider["selected_model"] = chosen
            provider["models"] = models
        elif chosen:
            # user typed a custom name not in the list
            provider["selected_model"] = chosen
            provider["models"] = models + [chosen]
        else:
            provider["selected_model"] = models[0]
            provider["models"] = models
    else:
        print(
            _style(
                "  Could not auto-detect models (endpoint may not support listing).",
                YELLOW,
            )
        )
        model_name = _input("  Enter a model name manually", "")
        if not model_name:
            model_name = _input("  Model name (required)", "gpt-4o")
        provider["selected_model"] = model_name
        provider["models"] = [model_name]

    # 6) Save
    cfg["providers"][name] = provider
    cfg["active_provider"] = name
    save_config(cfg)

    print()
    print(
        _style(
            f"  ✓ Provider '{name}' saved.  Model: {provider['selected_model']}", GREEN
        )
    )
    return True


# ---- quick setup from env vars / CLI args -----------------------------------


def _ensure_provider_from_env(cfg: dict[str, Any]) -> None:
    """If no providers exist but env vars are set, create one automatically."""
    if cfg["providers"]:
        return
    api_key = os.environ.get("OPENAI_API_KEY", "")
    base_url = os.environ.get("OPENAI_BASE_URL", "https://api.openai.com/v1")
    model = os.environ.get("BIMO_MODEL", "gpt-4o")
    if not api_key:
        return
    name = "openai" if base_url == "https://api.openai.com/v1" else "custom"
    provider: dict[str, Any] = {
        "base_url": base_url,
        "api_key": api_key,
        "models": [model],
        "selected_model": model,
    }
    cfg["providers"][name] = provider
    cfg["active_provider"] = name
    save_config(cfg)
    print(
        _style(f"  Auto-configured provider '{name}' from environment variables.", DIM)
    )


# ---- config management commands ---------------------------------------------


def _get_active_provider(cfg: dict[str, Any]) -> tuple[str, dict[str, Any]] | None:
    name = cfg.get("active_provider")
    if name and name in cfg.get("providers", {}):
        return name, cfg["providers"][name]
    return None


def _cmd_providers(cfg: dict[str, Any]) -> None:
    """List all configured providers."""
    providers = cfg.get("providers", {})
    active = cfg.get("active_provider")
    if not providers:
        print(
            _style("  No providers configured. Use /provider add to set one up.", DIM)
        )
        return
    print()
    print(_style("  Providers:", BOLD))
    for name, p in providers.items():
        marker = _style(" ◀ active", GREEN) if name == active else ""
        model = p.get("selected_model", "?")
        url = p.get("base_url", "?")
        n_models = len(p.get("models", []))
        print(f"    {_style(name, BOLD)}{marker}")
        print(f"      URL:    {url}")
        print(f"      Model:  {model}  ({n_models} available)")
    print()


def _cmd_provider_add(cfg: dict[str, Any]) -> None:
    _setup_provider_interactive(cfg)


def _cmd_provider_rm(cfg: dict[str, Any], args: str) -> None:
    name = args.strip()
    if not name:
        print(_style("  Usage: /provider rm <name>", DIM))
        return
    if name not in cfg.get("providers", {}):
        print(_style(f"  Provider '{name}' not found.", RED))
        return
    confirm = _input(f"  Remove provider '{name}'? [y/N]", "n")
    if confirm.lower() not in ("y", "yes"):
        print(_style("  Cancelled.", DIM))
        return
    del cfg["providers"][name]
    if cfg.get("active_provider") == name:
        cfg["active_provider"] = next(iter(cfg["providers"]), None)
    save_config(cfg)
    print(_style(f"  ✓ Provider '{name}' removed.", GREEN))


def _cmd_provider_use(cfg: dict[str, Any], args: str) -> None:
    name = args.strip()
    if not name:
        print(_style("  Usage: /provider use <name>", DIM))
        return
    if name not in cfg.get("providers", {}):
        available = ", ".join(cfg.get("providers", {}).keys()) or "(none)"
        print(_style(f"  Provider '{name}' not found.  Available: {available}", RED))
        return
    cfg["active_provider"] = name
    save_config(cfg)
    p = cfg["providers"][name]
    print(
        _style(
            f"  ✓ Switched to provider '{name}' (model: {p.get('selected_model', '?')}).",
            GREEN,
        )
    )


def _cmd_model(cfg: dict[str, Any], args: str, *, refresh: bool = False) -> None:
    """List / switch / refresh models for the active provider."""
    pair = _get_active_provider(cfg)
    if not pair:
        print(
            _style(
                "  No active provider. Use /provider add or /provider use first.", DIM
            )
        )
        return
    pname, provider = pair

    if args.strip().lower() == "refresh" or refresh:
        print(_style("  Fetching models from API…", DIM))
        models = fetch_models(provider)
        if models:
            provider["models"] = models
            save_config(cfg)
            print(_style(f"  ✓ Found {len(models)} models.", GREEN))
        else:
            print(_style("  Could not fetch models from this provider.", YELLOW))
            return

    models = provider.get("models", [])
    current = provider.get("selected_model", "?")

    if not models:
        print(_style(f"  No models known for '{pname}'. Use /model refresh.", DIM))
        return

    print()
    print(_style(f"  Models for provider '{pname}' (current: {current}):", BOLD))
    chosen = _choose_from_list(
        models, "  Pick a model (or press Enter to keep current)"
    )
    if chosen and chosen in models:
        provider["selected_model"] = chosen
        save_config(cfg)
        print(_style(f"  ✓ Active model set to '{chosen}'.", GREEN))
    elif chosen and chosen not in models:
        # manual entry — add to list
        provider["models"].append(chosen)
        provider["selected_model"] = chosen
        save_config(cfg)
        print(_style(f"  ✓ Added and selected '{chosen}'.", GREEN))


def _cmd_config(cfg: dict[str, Any], project_dir: Path) -> None:
    """Show full configuration."""
    print()
    print(_style("  Configuration:", BOLD))
    print(f"    Config file:      {BIMO_CONFIG_FILE}")
    print(f"    Project dir:      {project_dir}")
    print(f"    Auto-approve:     {cfg.get('auto_approve', False)}")
    print(f"    Max iterations:   {cfg.get('max_iterations', MAX_AGENT_ITERATIONS)}")
    print(f"    Active provider:  {cfg.get('active_provider') or '(none)'}")
    _cmd_providers(cfg)


def _cmd_project(cfg: dict[str, Any], args: str) -> Path | None:
    """Show or change the project directory.  Returns new path if changed."""
    if args.strip():
        new_dir = Path(args.strip()).resolve()
        if not new_dir.is_dir():
            print(_style(f"  Directory does not exist: {new_dir}", RED))
            return None
        cfg["project_dir"] = str(new_dir)
        save_config(cfg)
        print(_style(f"  ✓ Project directory set to: {new_dir}", GREEN))
        return new_dir
    else:
        print(
            _style(
                f"  Project directory: {Path(cfg.get('project_dir') or Path.cwd()).resolve()}",
                BOLD,
            )
        )
        return None


def _cmd_autoapprove(cfg: dict[str, Any], args: str) -> None:
    val = args.strip().lower()
    if val in ("on", "true", "1", "yes"):
        cfg["auto_approve"] = True
    elif val in ("off", "false", "0", "no"):
        cfg["auto_approve"] = False
    else:
        current = cfg.get("auto_approve", False)
        cfg["auto_approve"] = not current
    save_config(cfg)
    state = "ON" if cfg["auto_approve"] else "OFF"
    print(_style(f"  Auto-approve: {state}", GREEN))


# ---- safety constants (unchanged logic, just data) -------------------------

IGNORED_DIRS: set[str] = {
    ".git",
    ".venv",
    "venv",
    ".env",
    "node_modules",
    "__pycache__",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "dist",
    "build",
    ".eggs",
}

SECRET_PATTERNS: list[str] = [
    ".env",
    ".env.local",
    ".env.production",
    ".env.staging",
    "credentials",
    "secret",
    "token",
    "private_key",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    ".jks",
]

DANGEROUS_CMD_PATTERNS: list[str] = [
    r"\brm\s+-rf\s+/",
    r"\bsudo\b",
    r"\bmkfs\b",
    r"\bdd\b.*of=/dev/",
    r"\bchmod\s+-R\s+777\s+/",
    r"\bchown\b.*\s+/",
    r"\b:(){ :\|:& };:",
    r"\bwget\b.*\|\s*sh",
    r"\bcurl\b.*\|\s*sh",
    r">\s*/dev/sd",
    r"\bshutdown\b",
    r"\breboot\b",
    r"\binit\s+[06]\b",
    r"\bkill\s+-9\s+-1\b",
    r"\bkillall\b",
]


# ---------------------------------------------------------------------------
# 2. File-safety utilities
# ---------------------------------------------------------------------------


class SafetyError(Exception):
    """Raised when a file operation is blocked by safety checks."""


def _normalize_path(path_str: str, project_dir: Path) -> Path:
    candidate = (project_dir / path_str).resolve()
    if not candidate.is_relative_to(project_dir):
        raise SafetyError(f"Path escapes project directory: {path_str!r} → {candidate}")
    return candidate


def _should_ignore(path: Path, project_dir: Path) -> bool:
    try:
        rel = path.relative_to(project_dir)
    except ValueError:
        return True
    for part in rel.parts:
        if part in IGNORED_DIRS:
            return True
        for pat in IGNORED_DIRS:
            if pat.startswith("*") and part.endswith(pat[1:]):
                return True
    name_lower = rel.name.lower()
    return any(pat.lower() in name_lower for pat in SECRET_PATTERNS)


def _is_dangerous_command(cmd: str) -> bool:
    return any(re.search(p, cmd, re.IGNORECASE) for p in DANGEROUS_CMD_PATTERNS)


def _confirm(prompt: str, auto_yes: bool = False) -> bool:
    if auto_yes:
        print(_style(f"  (auto-approved: {prompt})", DIM))
        return True
    try:
        answer = input(f"{prompt} [y/N] ").strip().lower()
        return answer in ("y", "yes")
    except (EOFError, KeyboardInterrupt):
        print()
        return False


def _truncate(text: str, max_lines: int = 500) -> str:
    lines = text.splitlines(keepends=True)
    if len(lines) <= max_lines:
        return text
    return (
        "".join(lines[:max_lines])
        + f"\n... ({len(lines) - max_lines} more lines truncated)\n"
    )


# ---------------------------------------------------------------------------
# 3. Tool implementations
# ---------------------------------------------------------------------------


def tool_list_files(args: dict[str, Any], project_dir: Path) -> str:
    root = _normalize_path(args.get("path", "."), project_dir)
    max_depth = int(args.get("max_depth", 4))
    lines: list[str] = []

    def _walk(current: Path, prefix: str, depth: int) -> None:
        if depth > max_depth:
            return
        try:
            entries = sorted(
                current.iterdir(), key=lambda p: (not p.is_dir(), p.name.lower())
            )
        except PermissionError:
            return
        visible = [e for e in entries if not _should_ignore(e, project_dir)]
        for i, entry in enumerate(visible):
            is_last = i == len(visible) - 1
            connector = "└── " if is_last else "├── "
            name = entry.name + ("/" if entry.is_dir() else "")
            lines.append(prefix + connector + name)
            if entry.is_dir():
                extension = "    " if is_last else "│   "
                _walk(entry, prefix + extension, depth + 1)

    rel = root.relative_to(project_dir) if root != project_dir else Path(".")
    lines.append(str(rel) + "/")
    _walk(root, "", 0)
    return "\n".join(lines)


def tool_read_file(args: dict[str, Any], project_dir: Path) -> str:
    path = _normalize_path(args["file_path"], project_dir)
    if not path.exists():
        return f"Error: file not found: {args['file_path']}"
    if not path.is_file():
        return f"Error: not a regular file: {args['file_path']}"
    if _should_ignore(path, project_dir):
        return (
            f"Warning: {args['file_path']} is in an ignored or secret path — skipping."
        )
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        return f"Error reading file: {e}"
    numbered = text.splitlines(keepends=True)
    start = max(1, int(args.get("start_line", 1)))
    end = min(len(numbered), int(args.get("end_line", len(numbered))))
    return "\n".join(
        f"{i + 1:>6}\t{numbered[i].rstrip()}" for i in range(start - 1, end)
    )


def tool_search_files(args: dict[str, Any], project_dir: Path) -> str:
    query = args["query"]
    glob_pattern = args.get("glob", "**/*")
    max_results = int(args.get("max_results", 50))
    results: list[str] = []
    count = 0
    for path in project_dir.glob(glob_pattern):
        if not path.is_file() or _should_ignore(path, project_dir):
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        for i, line in enumerate(text.splitlines(), 1):
            if query.lower() in line.lower():
                results.append(f"{path.relative_to(project_dir)}:{i}: {line.rstrip()}")
                count += 1
                if count >= max_results:
                    results.append(f"\n... (stopped after {max_results} matches)")
                    return "\n".join(results)
    return "\n".join(results) if results else f"No matches found for '{query}'."


def tool_write_file(args: dict[str, Any], project_dir: Path) -> str:
    path = _normalize_path(args["file_path"], project_dir)
    content = args["content"]
    if _should_ignore(path, project_dir):
        raise SafetyError(f"Cannot write to ignored/secret path: {args['file_path']}")
    if path.exists():
        try:
            old = path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            old = ""
        diff = "\n".join(
            unified_diff(
                old.splitlines(keepends=True),
                content.splitlines(keepends=True),
                fromfile=f"a/{args['file_path']}",
                tofile=f"b/{args['file_path']}",
                lineterm="",
            )
        )
        if diff:
            print(_style("\n  Diff:", BOLD))
            for line in diff.splitlines():
                if line.startswith("+") and not line.startswith("+++"):
                    print(_style(f"  {line}", GREEN))
                elif line.startswith("-") and not line.startswith("---"):
                    print(_style(f"  {line}", RED))
                else:
                    print(f"  {line}")
    if not _confirm(
        f"  Write {args['file_path']}?", auto_yes=args.get("_auto_yes", False)
    ):
        return "Write cancelled by user."
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    return f"File written: {args['file_path']} ({len(content)} chars)"


def tool_edit_file(args: dict[str, Any], project_dir: Path) -> str:
    path = _normalize_path(args["file_path"], project_dir)
    if not path.exists():
        return f"Error: file not found: {args['file_path']}"
    if _should_ignore(path, project_dir):
        raise SafetyError(f"Cannot edit ignored/secret path: {args['file_path']}")
    old_text, new_text = args["old_text"], args["new_text"]
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        return f"Error reading file: {e}"
    count = content.count(old_text)
    if count == 0:
        return f"Error: old_text not found in {args['file_path']}. Make sure the text matches exactly."
    if count > 1 and not args.get("allow_multiple", False):
        return f"Error: old_text found {count} times in {args['file_path']}. Provide more context to make it unique."
    new_content = content.replace(
        old_text, new_text, 1 if not args.get("allow_multiple", False) else -1
    )
    diff = "\n".join(
        unified_diff(
            content.splitlines(keepends=True),
            new_content.splitlines(keepends=True),
            fromfile=f"a/{args['file_path']}",
            tofile=f"b/{args['file_path']}",
            lineterm="",
        )
    )
    if not diff:
        return "No changes — old_text and new_text are identical."
    print(_style("\n  Diff:", BOLD))
    for line in diff.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            print(_style(f"  {line}", GREEN))
        elif line.startswith("-") and not line.startswith("---"):
            print(_style(f"  {line}", RED))
        else:
            print(f"  {line}")
    if not _confirm(
        f"  Apply edit to {args['file_path']}?", auto_yes=args.get("_auto_yes", False)
    ):
        return "Edit cancelled by user."
    path.write_text(new_content, encoding="utf-8")
    return f"File edited: {args['file_path']}"


def tool_delete_file(args: dict[str, Any], project_dir: Path) -> str:
    path = _normalize_path(args["file_path"], project_dir)
    if not path.exists():
        return f"Error: file not found: {args['file_path']}"
    if _should_ignore(path, project_dir):
        raise SafetyError(f"Cannot delete ignored/secret path: {args['file_path']}")
    if path.is_dir():
        return f"Error: {args['file_path']} is a directory. Use run_command with 'rm -r' for directories."
    if not _confirm(f"  DELETE {args['file_path']}? This cannot be undone."):
        return "Delete cancelled by user."
    path.unlink()
    return f"File deleted: {args['file_path']}"


def tool_run_command(args: dict[str, Any], project_dir: Path) -> str:
    cmd = args["command"]
    if _is_dangerous_command(cmd):
        if not _confirm(
            f"  DANGEROUS command detected: {cmd!r}. Run anyway?", auto_yes=False
        ):
            return "Command cancelled by user (dangerous)."
    timeout = min(int(args.get("timeout", 120)), 600)
    try:
        result = subprocess.run(
            cmd,
            shell=True,
            cwd=str(project_dir),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        parts: list[str] = []
        if result.stdout:
            parts.append(f"STDOUT:\n{_truncate(result.stdout)}")
        if result.stderr:
            parts.append(f"STDERR:\n{_truncate(result.stderr)}")
        parts.append(f"EXIT CODE: {result.returncode}")
        return "\n".join(parts) if parts else "(no output)"
    except subprocess.TimeoutExpired:
        return f"Error: command timed out after {timeout}s."
    except Exception as e:
        return f"Error running command: {e}"


# ---------------------------------------------------------------------------
# 4. Tool schemas (OpenAI function-calling format)
# ---------------------------------------------------------------------------

TOOL_SCHEMAS: list[dict[str, Any]] = [
    {
        "type": "function",
        "function": {
            "name": "list_files",
            "description": "Show the project directory tree. Defaults to current directory.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative directory path. Defaults to '.'.",
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Max depth. Defaults to 4.",
                    },
                },
                "required": [],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a file's contents with line numbers.",
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Relative path to the file.",
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line (1-based). Defaults to 1.",
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line (inclusive). Defaults to EOF.",
                    },
                },
                "required": ["file_path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "search_files",
            "description": "Search for text across project files. Returns matching lines with file and line number.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for (case-insensitive).",
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob filter. Defaults to '**/*'.",
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Max results. Defaults to 50.",
                    },
                },
                "required": ["query"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Create a new file or completely replace an existing file's contents.",
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Relative path for the file.",
                    },
                    "content": {
                        "type": "string",
                        "description": "The full content to write.",
                    },
                },
                "required": ["file_path", "content"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Replace a specific section of a file. old_text must match exactly.",
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Relative path to the file.",
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find and replace.",
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The replacement text.",
                    },
                    "allow_multiple": {
                        "type": "boolean",
                        "description": "Replace all occurrences.",
                    },
                },
                "required": ["file_path", "old_text", "new_text"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "delete_file",
            "description": "Delete a file. Always requires user confirmation.",
            "parameters": {
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Relative path to the file to delete.",
                    },
                },
                "required": ["file_path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_command",
            "description": "Run a shell command in the project directory. Dangerous commands require confirmation.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute.",
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds. Defaults to 120, max 600.",
                    },
                },
                "required": ["command"],
            },
        },
    },
]

TOOL_DISPATCH: dict[str, Any] = {
    "list_files": tool_list_files,
    "read_file": tool_read_file,
    "search_files": tool_search_files,
    "write_file": tool_write_file,
    "edit_file": tool_edit_file,
    "delete_file": tool_delete_file,
    "run_command": tool_run_command,
}


# ---------------------------------------------------------------------------
# 5. System prompt
# ---------------------------------------------------------------------------

SYSTEM_PROMPT = """\
You are Bimo, a skilled local AI coding agent. You help users understand, build, \
debug, and modify software projects by inspecting the codebase and making precise changes.

## Core principles

1. **Inspect before acting.** Always list files and read relevant code before making assumptions \
about the project structure or behavior.
2. **Read before editing.** Always `read_file` the exact file you intend to modify. Never edit \
blindly.
3. **Minimal changes.** Make the smallest targeted change that solves the problem. \
Do not refactor unrelated code.
4. **Use tools, don't guess.** If you're unsure about a file's content, a function's signature, \
or how something works — read it.
5. **Verify your work.** After making changes, run relevant tests or commands to confirm \
the change works. If something fails, investigate and fix it.
6. **Stay in the project.** All your file operations are restricted to the current project \
directory. Never attempt to access files outside it.
7. **Explain your reasoning.** When you make a change, briefly explain *why* — what problem \
it solves and how.

## Working style

- Be concise and direct. The user wants results, not essays.
- When encountering errors, inspect the error output carefully and fix the root cause.
- When a task is ambiguous, inspect the project first to understand context, then proceed \
with the most reasonable interpretation.
- For complex tasks, break them into steps and execute them methodically.
- If you need to run a build or test command to verify your changes, do it.

## Tool usage

You have access to tools for listing files, reading files, searching code, writing/editing \
files, deleting files, and running shell commands. Use them proactively.

When writing or editing files:
- Show awareness of the existing code style and conventions.
- Preserve existing formatting unless the change requires altering it.
- After editing, verify the result if possible (e.g. by running a linter or tests).
"""


# ---------------------------------------------------------------------------
# 6. Agent loop
# ---------------------------------------------------------------------------


def _tool_arg_repr(v: Any) -> str:
    s = repr(v)
    return s[:57] + "..." if len(s) > 60 else s


def _execute_tool_call(
    tool_name: str,
    tool_args: dict[str, Any],
    project_dir: Path,
    auto_yes: bool,
) -> str:
    tool_fn = TOOL_DISPATCH.get(tool_name)
    if tool_fn is None:
        return f"Error: unknown tool '{tool_name}'"
    if tool_name in ("write_file", "edit_file"):
        tool_args["_auto_yes"] = auto_yes
    try:
        return tool_fn(tool_args, project_dir)
    except SafetyError as e:
        return f"Safety error: {e}"
    except Exception as e:
        return f"Tool error ({tool_name}): {e}"


def run_agent(
    user_message: str,
    messages: list[dict[str, Any]],
    *,
    client: Any,
    model: str,
    project_dir: Path,
    auto_yes: bool,
    max_iterations: int,
) -> str:
    messages.append({"role": "user", "content": user_message})

    for iteration in range(max_iterations):
        print(_style(f"\n  ⏳ Thinking... (iteration {iteration + 1})", DIM))
        try:
            response = client.chat.completions.create(
                model=model,
                messages=messages,
                tools=TOOL_SCHEMAS,
                tool_choice="auto",
            )
        except Exception as e:
            error_msg = f"API error: {e}"
            print(_style(f"  {error_msg}", RED))
            messages.append({"role": "assistant", "content": error_msg})
            return error_msg

        message = response.choices[0].message

        if not message.tool_calls:
            final_text = message.content or ""
            messages.append({"role": "assistant", "content": final_text})
            return final_text

        assistant_msg: dict[str, Any] = {
            "role": "assistant",
            "content": message.content or "",
            "tool_calls": [],
        }
        for tc in message.tool_calls:
            assistant_msg["tool_calls"].append(
                {
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function.name,
                        "arguments": tc.function.arguments,
                    },
                }
            )
        messages.append(assistant_msg)

        for tc in message.tool_calls:
            tool_name = tc.function.name
            try:
                tool_args = json.loads(tc.function.arguments)
            except json.JSONDecodeError:
                tool_args = {}

            arg_str = ", ".join(
                f"{k}={_tool_arg_repr(v)}"
                for k, v in tool_args.items()
                if k != "_auto_yes"
            )
            print(_style(f"  🔧 {tool_name}({arg_str})", BOLD, CYAN))

            result = _execute_tool_call(tool_name, tool_args, project_dir, auto_yes)
            preview = result if len(result) <= 300 else result[:300] + "..."
            print(_style(f"  📋 {preview}", DIM))

            messages.append({"role": "tool", "tool_call_id": tc.id, "content": result})

    # Exhausted iterations — ask model for final answer
    messages.append(
        {
            "role": "user",
            "content": f"[Bimo] Max iterations ({max_iterations}) reached. Give your final answer now.",
        }
    )
    try:
        response = client.chat.completions.create(
            model=model,
            messages=messages,
            tools=TOOL_SCHEMAS,
            tool_choice="auto",
        )
        final = response.choices[0].message.content or ""
        messages.append({"role": "assistant", "content": final})
        return final
    except Exception:
        return "[Bimo] Reached maximum iterations. Please rephrase or be more specific."


# ---------------------------------------------------------------------------
# 7. CLI — REPL with slash commands
# ---------------------------------------------------------------------------


def _print_banner(provider_name: str, model: str, project_dir: Path) -> None:
    print()
    print(_style("  ╔══════════════════════════════════════╗", BOLD, CYAN))
    print(_style("  ║          B I M O                    ║", BOLD, CYAN))
    print(_style("  ║   Local AI Coding Agent             ║", BOLD, CYAN))
    print(_style("  ╚══════════════════════════════════════╝", BOLD, CYAN))
    print()
    print(f"  Provider:   {_style(provider_name, BOLD)}")
    print(f"  Model:      {_style(model, BOLD)}")
    print(f"  Project:    {project_dir}")
    print()
    print(f"  Type {_style('/help', BOLD)} for commands, or just start chatting.")
    print()


def _print_help() -> None:
    print()
    print(_style("  Agent:", BOLD))
    print(f"    Just type your request in natural language.")
    print()
    print(_style("  Commands:", BOLD))
    print(f"    {_style('/help', CYAN)}              Show this help message")
    print(f"    {_style('/clear', CYAN)}             Clear conversation history")
    print(f"    {_style('/status', CYAN)}            Show current configuration")
    print(
        f"    {_style('/config', CYAN)}            Show full config and all providers"
    )
    print(f"    {_style('/providers', CYAN)}         List configured providers")
    print(f"    {_style('/provider add', CYAN)}      Add a new provider (interactive)")
    print(f"    {_style('/provider rm <n>', CYAN)}   Remove a provider")
    print(f"    {_style('/provider use <n>', CYAN)}  Switch active provider")
    print(
        f"    {_style('/model', CYAN)}             List & select model for active provider"
    )
    print(
        f"    {_style('/model refresh', CYAN)}     Re-fetch available models from API"
    )
    print(f"    {_style('/project [dir]', CYAN)}     Show or change project directory")
    print(
        f"    {_style('/autoapprove', CYAN)}       Toggle auto-approve for file changes"
    )
    print(f"    {_style('/exit, /quit', CYAN)}       Exit Bimo")
    print()


# ---------------------------------------------------------------------------
# 8. main()
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="bimo",
        description="Bimo — a local CLI coding agent powered by any OpenAI-compatible API.",
    )
    parser.add_argument(
        "--project-dir",
        default=None,
        help="Project directory (default: current working directory)",
    )
    parser.add_argument(
        "--yes",
        "-y",
        action="store_true",
        default=False,
        help="Auto-approve normal file changes",
    )
    parser.add_argument(
        "--provider",
        default=None,
        help="Use a specific saved provider by name (for this session)",
    )
    parser.add_argument(
        "--model",
        default=None,
        help="Override the model for this session",
    )
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=None,
        help=f"Maximum agent loop iterations (default: {MAX_AGENT_ITERATIONS})",
    )
    args = parser.parse_args()

    # Load persisted config
    cfg = load_config()

    # Auto-configure from env vars if nothing is set up yet
    _ensure_provider_from_env(cfg)

    # Resolve project directory: CLI arg > config > cwd
    if args.project_dir:
        project_dir = Path(args.project_dir).resolve()
    elif cfg.get("project_dir"):
        project_dir = Path(cfg["project_dir"]).resolve()
    else:
        project_dir = Path.cwd().resolve()

    if not project_dir.is_dir():
        print(_style(f"  Error: project directory does not exist: {project_dir}", RED))
        sys.exit(1)

    auto_yes = args.yes
    max_iterations = args.max_iterations or cfg.get(
        "max_iterations", MAX_AGENT_ITERATIONS
    )

    # Resolve active provider
    provider_name: str | None = args.provider or cfg.get("active_provider")
    provider: dict[str, Any] | None = None
    if provider_name and provider_name in cfg.get("providers", {}):
        provider = cfg["providers"][provider_name]
    elif cfg.get("providers"):
        # fall back to first available
        provider_name = next(iter(cfg["providers"]))
        provider = cfg["providers"][provider_name]

    # Resolve model: CLI override > provider's selected_model
    if provider:
        model = args.model or provider.get("selected_model", "gpt-4o")
    else:
        model = args.model or "gpt-4o"

    # ---- Setup wizard if no provider configured ----
    if not provider:
        print()
        print(_style("  Welcome to Bimo!", BOLD, CYAN))
        print()
        print(_style("  No providers configured yet. Let's set one up.", DIM))
        _setup_provider_interactive(cfg, is_first=True)
        # Re-read after setup
        if cfg.get("active_provider") and cfg["active_provider"] in cfg.get(
            "providers", {}
        ):
            provider_name = cfg["active_provider"]
            provider = cfg["providers"][provider_name]
            model = provider.get("selected_model", "gpt-4o")
        else:
            print(_style("  No provider was set up. Exiting.", RED))
            sys.exit(0)

    # ---- Create client and test connection ----
    client = _make_client(provider)
    print(_style("  Connecting to API…", DIM))
    try:
        test = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Say 'ok' in one word."}],
            max_tokens=5,
        )
        _ = test.choices[0].message.content
        print(_style("  ✓ Connected.", GREEN))
    except Exception as e:
        print(_style(f"  ✗ Connection failed: {e}", RED))
        print()
        print("  Troubleshooting:")
        print("    - Check your API key and base URL")
        print("    - Verify the model name is correct (use /model refresh)")
        print("    - For Ollama, ensure it's running: ollama serve")
        print("    - Use /provider add to reconfigure")
        print()
        # Don't exit — let them fix via /provider commands

    # ---- Print banner ----
    _print_banner(provider_name or "?", model, project_dir)

    # ---- Conversation history ----
    messages: list[dict[str, Any]] = [
        {"role": "system", "content": SYSTEM_PROMPT},
    ]

    # ---- REPL loop ----
    while True:
        try:
            user_input = input(_style("  you ▸ ", BOLD, GREEN)).strip()
        except (EOFError, KeyboardInterrupt):
            print()
            print(_style("  Goodbye! 👋", DIM))
            break

        if not user_input:
            continue

        # ---- Slash commands ----
        if user_input.startswith("/"):
            parts = user_input.split(maxsplit=1)
            cmd = parts[0].lower()
            cmd_args = parts[1] if len(parts) > 1 else ""

            if cmd in ("/exit", "/quit", "/q"):
                print(_style("  Goodbye! 👋", DIM))
                break

            elif cmd == "/help":
                _print_help()

            elif cmd == "/clear":
                messages.clear()
                messages.append({"role": "system", "content": SYSTEM_PROMPT})
                print(_style("  Conversation cleared.", DIM))

            elif cmd == "/status":
                print()
                print(_style("  Status:", BOLD))
                print(f"    Provider:       {provider_name or '(none)'}")
                print(f"    Model:          {model}")
                print(
                    f"    Base URL:       {provider.get('base_url', '?') if provider else '?'}"
                )
                print(f"    Project dir:    {project_dir}")
                print(f"    Auto-approve:   {cfg.get('auto_approve', False)}")
                print(f"    Max iterations: {max_iterations}")
                print(f"    Messages:       {len(messages)}")
                print(f"    Config file:    {BIMO_CONFIG_FILE}")
                print()

            elif cmd == "/config":
                _cmd_config(cfg, project_dir)

            elif cmd == "/providers":
                _cmd_providers(cfg)

            elif cmd == "/provider":
                sub = cmd_args.strip().split(maxsplit=1)
                sub_cmd = sub[0].lower() if sub else ""
                sub_args = sub[1] if len(sub) > 1 else ""

                if sub_cmd == "add":
                    _cmd_provider_add(cfg)
                    # Refresh client if provider changed
                    new_pair = _get_active_provider(cfg)
                    if new_pair:
                        provider_name, provider = new_pair
                        model = provider.get("selected_model", model)
                        client = _make_client(provider)
                elif sub_cmd in ("rm", "remove", "delete"):
                    _cmd_provider_rm(cfg, sub_args)
                    new_pair = _get_active_provider(cfg)
                    if new_pair:
                        provider_name, provider = new_pair
                        model = provider.get("selected_model", model)
                        client = _make_client(provider)
                    elif not cfg.get("providers"):
                        provider_name = None
                        provider = None
                elif sub_cmd in ("use", "switch"):
                    _cmd_provider_use(cfg, sub_args)
                    new_pair = _get_active_provider(cfg)
                    if new_pair:
                        provider_name, provider = new_pair
                        model = provider.get("selected_model", model)
                        client = _make_client(provider)
                else:
                    print(_style("  Usage: /provider [add|rm|use] [name]", DIM))

            elif cmd == "/model":
                sub = cmd_args.strip().lower()
                if sub == "refresh":
                    _cmd_model(cfg, "refresh", refresh=True)
                else:
                    _cmd_model(cfg, cmd_args)
                # Sync model after selection
                new_pair = _get_active_provider(cfg)
                if new_pair:
                    provider_name, provider = new_pair
                    model = args.model or provider.get("selected_model", model)

            elif cmd == "/project":
                result = _cmd_project(cfg, cmd_args)
                if result:
                    project_dir = result

            elif cmd == "/autoapprove":
                _cmd_autoapprove(cfg, cmd_args)

            else:
                print(
                    _style(
                        f"  Unknown command: {cmd}. Type /help for available commands.",
                        DIM,
                    )
                )

            continue

        # ---- Run agent ----
        if not provider:
            print(
                _style(
                    "  No provider configured. Use /provider add to set one up.", RED
                )
            )
            continue

        try:
            response = run_agent(
                user_input,
                messages,
                client=client,
                model=model,
                project_dir=project_dir,
                auto_yes=auto_yes,
                max_iterations=max_iterations,
            )
            print()
            print(_style("  bimo ▸ ", BOLD, MAGENTA) + response)
        except KeyboardInterrupt:
            print(_style("\n  (interrupted)", DIM))
        except Exception as e:
            print(_style(f"\n  Error: {e}", RED))

    print()


if __name__ == "__main__":
    main()
