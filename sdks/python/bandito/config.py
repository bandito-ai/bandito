"""Config loader — reads ~/.bandito/config.toml and env vars."""

from __future__ import annotations

import logging
import os
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

logger = logging.getLogger(__name__)

CONFIG_DIR = Path.home() / ".bandito"
CONFIG_FILE = CONFIG_DIR / "config.toml"

DEFAULT_BASE_URL = "https://bandito-api.onrender.com"


@dataclass
class S3Config:
    bucket: str
    prefix: str = "bandito"
    region: str = "us-east-1"
    endpoint: str | None = None  # for MinIO / LocalStack / custom S3-compatible stores


@dataclass
class JudgeConfig:
    api_key: str | None = None
    model: str = "gpt-4o-mini"


@dataclass
class BanditoConfig:
    api_key: str | None = None
    base_url: str = DEFAULT_BASE_URL
    data_storage: str = "local"  # "local", "cloud", or "s3"
    s3: S3Config | None = None
    judge: JudgeConfig = field(default_factory=JudgeConfig)


def _escape_toml_value(value: str) -> str:
    """Escape a string for use as a TOML basic string value."""
    return value.replace("\\", "\\\\").replace('"', '\\"')


def load_config() -> BanditoConfig:
    """Load config from TOML file, falling back to env vars."""
    config = BanditoConfig()

    # TOML file first
    if CONFIG_FILE.exists():
        try:
            with open(CONFIG_FILE, "rb") as f:
                data = tomllib.load(f)
            config.api_key = data.get("api_key", config.api_key)
            config.base_url = data.get("base_url", config.base_url)
            config.data_storage = data.get("data_storage", config.data_storage)
            s3_data = data.get("s3", {})
            bucket = str(s3_data.get("bucket", "")).strip()
            if bucket:
                endpoint_raw = s3_data.get("endpoint")
                config.s3 = S3Config(
                    bucket=bucket,
                    prefix=str(s3_data.get("prefix", "bandito")).strip() or "bandito",
                    region=str(s3_data.get("region", "us-east-1")).strip() or "us-east-1",
                    endpoint=str(endpoint_raw).strip() or None if endpoint_raw else None,
                )
            judge_data = data.get("judge", {})
            if judge_data.get("api_key"):
                config.judge.api_key = judge_data["api_key"]
            if judge_data.get("model"):
                config.judge.model = judge_data["model"]
        except tomllib.TOMLDecodeError:
            logger.warning(
                "Failed to parse %s — falling back to env vars", CONFIG_FILE
            )

    # Env vars override
    env_key = os.environ.get("BANDITO_API_KEY")
    if env_key:
        config.api_key = env_key
    env_url = os.environ.get("BANDITO_BASE_URL")
    if env_url:
        config.base_url = env_url
    env_storage = os.environ.get("BANDITO_DATA_STORAGE")
    if env_storage:
        config.data_storage = env_storage
    # S3 env vars (useful for container deployments without a config file)
    env_s3_bucket = os.environ.get("BANDITO_S3_BUCKET", "").strip()
    if env_s3_bucket:
        config.s3 = S3Config(
            bucket=env_s3_bucket,
            prefix=os.environ.get("BANDITO_S3_PREFIX", config.s3.prefix if config.s3 else "bandito").strip() or "bandito",
            region=os.environ.get("BANDITO_S3_REGION", config.s3.region if config.s3 else "us-east-1").strip() or "us-east-1",
            endpoint=os.environ.get("BANDITO_S3_ENDPOINT", config.s3.endpoint if config.s3 else None) or None,
        )
        # Setting BANDITO_S3_BUCKET implicitly activates s3 mode unless
        # the user has explicitly chosen a different mode via BANDITO_DATA_STORAGE.
        if not os.environ.get("BANDITO_DATA_STORAGE"):
            config.data_storage = "s3"
    elif config.s3:
        if os.environ.get("BANDITO_S3_PREFIX"):
            config.s3.prefix = os.environ["BANDITO_S3_PREFIX"].strip() or config.s3.prefix
        if os.environ.get("BANDITO_S3_REGION"):
            config.s3.region = os.environ["BANDITO_S3_REGION"].strip() or config.s3.region
        if os.environ.get("BANDITO_S3_ENDPOINT"):
            config.s3.endpoint = os.environ["BANDITO_S3_ENDPOINT"].strip() or config.s3.endpoint
    env_judge_key = os.environ.get("JUDGE_API_KEY")
    if env_judge_key:
        config.judge.api_key = env_judge_key

    return config


def save_config(
    api_key: str,
    base_url: str = DEFAULT_BASE_URL,
    data_storage: str = "local",
) -> None:
    """Write config to ~/.bandito/config.toml."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True, mode=0o700)
    lines = [f'api_key = "{_escape_toml_value(api_key)}"']
    if base_url != DEFAULT_BASE_URL:
        lines.append(f'base_url = "{_escape_toml_value(base_url)}"')
    lines.append(f'data_storage = "{_escape_toml_value(data_storage)}"')
    CONFIG_FILE.write_text("\n".join(lines) + "\n")
    CONFIG_FILE.chmod(0o600)
