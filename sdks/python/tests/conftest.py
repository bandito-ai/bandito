"""Shared fixtures for SDK tests."""

import pytest

from bandito.models import Arm


# ── Standard 3-arm bandit setup ──────────────────────────────────────────

ARM_DATA = [
    {"arm_id": 1, "model_name": "gpt-4", "model_provider": "OpenAI", "system_prompt": "You are helpful", "is_prompt_templated": False},
    {"arm_id": 2, "model_name": "claude-sonnet", "model_provider": "Anthropic", "system_prompt": "You are helpful", "is_prompt_templated": False},
    {"arm_id": 3, "model_name": "gpt-4", "model_provider": "OpenAI", "system_prompt": "Be concise", "is_prompt_templated": True},
]

# 2 models, 2 prompts → dims = 3*2 + 2 = 8
EXPECTED_DIMS = 8


@pytest.fixture
def arms():
    return [Arm(**a) for a in ARM_DATA]


def make_sync_response(bandits=None, *, budget=None, total_cost=None):
    """Build a mock sync response matching the backend SyncResponse schema."""
    if bandits is None:
        d = EXPECTED_DIMS
        chol = [0.0] * (d * d)
        for i in range(d):
            chol[i * d + i] = 1.0
        bandits = [{
            "bandit_id": 1,
            "name": "my-chatbot",
            "type": "online",
            "cost_importance": 2,
            "latency_importance": 3,
            "optimization_mode": "base",
            "total_pull_count": 0,
            "avg_latency_last_n": None,
            "budget": budget,
            "total_cost": total_cost,
            "theta": [0.0] * d,
            "cholesky": chol,
            "dimensions": d,
            "arms": [
                {**a, "is_active": True, "avg_latency_last_n": None}
                for a in ARM_DATA
            ],
        }]
    return {
        "bandits": bandits,
        "server_time": "2025-01-01T00:00:00Z",
    }
