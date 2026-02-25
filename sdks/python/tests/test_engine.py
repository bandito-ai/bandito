"""Tests for the Rust engine PyO3 bindings (bandito_engine)."""

import json

import pytest

from bandito_engine import BanditEngine
from tests.conftest import ARM_DATA, EXPECTED_DIMS


def _make_bandit_json(*, dims=EXPECTED_DIMS, arms=None, theta=None, optimization_mode="base"):
    """Build a minimal bandit JSON string for engine tests."""
    if arms is None:
        arms = [
            {**a, "is_active": True, "avg_latency_last_n": None}
            for a in ARM_DATA
        ]
    if theta is None:
        theta = [0.0] * dims
    chol = [0.0] * (dims * dims)
    for i in range(dims):
        chol[i * dims + i] = 1.0
    return json.dumps({
        "bandit_id": 1,
        "name": "test-bandit",
        "theta": theta,
        "cholesky": chol,
        "dimensions": dims,
        "optimization_mode": optimization_mode,
        "avg_latency_last_n": 500.0,
        "arms": arms,
    })


class TestBanditEngine:
    def test_create(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        assert engine.bandit_id == 1
        assert engine.bandit_name == "test-bandit"
        assert engine.dimensions == EXPECTED_DIMS
        assert engine.num_arms == 3

    def test_pull_returns_json(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        result = json.loads(engine.pull(query_length=100))
        assert "arm_id" in result
        assert "scores" in result
        assert len(result["scores"]) == 3

    def test_pull_deterministic(self):
        e1 = BanditEngine(_make_bandit_json(), seed=42)
        e2 = BanditEngine(_make_bandit_json(), seed=42)
        r1 = json.loads(e1.pull(query_length=100))
        r2 = json.loads(e2.pull(query_length=100))
        assert r1["arm_id"] == r2["arm_id"]
        assert r1["scores"] == r2["scores"]

    def test_pull_with_exclude(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        result = json.loads(engine.pull(exclude_ids=[1, 2]))
        assert result["arm_id"] == 3
        assert len(result["scores"]) == 1

    def test_pull_all_excluded_raises(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        with pytest.raises(ValueError, match="excluded or inactive"):
            engine.pull(exclude_ids=[1, 2, 3])

    def test_update_from_sync(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        new_json = _make_bandit_json(theta=[0.5] * EXPECTED_DIMS, optimization_mode="explore")
        engine.update_from_sync(new_json)
        # Should still pull successfully
        result = json.loads(engine.pull())
        assert "arm_id" in result

    def test_get_arms_json(self):
        engine = BanditEngine(_make_bandit_json(), seed=42)
        arms = json.loads(engine.get_arms_json())
        assert len(arms) == 3
        assert all(a["is_active"] for a in arms)

    def test_inactive_arms(self):
        arms = [
            {**ARM_DATA[0], "is_active": True, "avg_latency_last_n": None},
            {**ARM_DATA[1], "is_active": False, "avg_latency_last_n": None},
            {**ARM_DATA[2], "is_active": True, "avg_latency_last_n": None},
        ]
        engine = BanditEngine(_make_bandit_json(arms=arms), seed=42)
        result = json.loads(engine.pull())
        # Only active arms should have scores
        assert len(result["scores"]) == 2
        assert "2" not in result["scores"]

    def test_empty_arms_raises(self):
        with pytest.raises(ValueError, match="No arms"):
            BanditEngine(_make_bandit_json(arms=[]))
