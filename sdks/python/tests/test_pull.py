"""Tests for the pull() method — local Thompson Sampling decisions."""

import pytest
import httpx
import respx

from bandito.client import BanditoClient
from bandito.models import PullResult
from tests.conftest import ARM_DATA, EXPECTED_DIMS, make_sync_response


BASE_URL = "http://test.local"
API_KEY = "bnd_test123"


def _connected_client(sync_data=None) -> BanditoClient:
    """Create a client that's already connected with mocked HTTP."""
    if sync_data is None:
        sync_data = make_sync_response()
    respx.post(f"{BASE_URL}/api/v1/sync/connect").mock(
        return_value=httpx.Response(200, json=sync_data)
    )
    client = BanditoClient(
        api_key=API_KEY,
        base_url=BASE_URL,
        store_path=":memory:",
    )
    client.connect()
    return client


class TestPull:
    @respx.mock
    def test_pull_returns_pull_result(self):
        client = _connected_client()
        try:
            result = client.pull("my-chatbot")
            assert isinstance(result, PullResult)
            assert result.bandit_name == "my-chatbot"
            assert result.bandit_id == 1
            assert result.arm is not None
            assert result.event_id  # non-empty UUID string
        finally:
            client.close()

    @respx.mock
    def test_pull_event_id_unique(self):
        client = _connected_client()
        try:
            r1 = client.pull("my-chatbot")
            r2 = client.pull("my-chatbot")
            assert r1.event_id != r2.event_id
        finally:
            client.close()

    @respx.mock
    def test_pull_scores_all_arms(self):
        client = _connected_client()
        try:
            result = client.pull("my-chatbot")
            assert len(result.scores) == 3
            assert set(result.scores.keys()) == {1, 2, 3}
        finally:
            client.close()

    @respx.mock
    def test_pull_winner_has_highest_score(self):
        client = _connected_client()
        try:
            result = client.pull("my-chatbot")
            winner_score = result.scores[result.arm.arm_id]
            assert winner_score == max(result.scores.values())
        finally:
            client.close()

    @respx.mock
    def test_pull_unknown_bandit_raises(self):
        client = _connected_client()
        try:
            with pytest.raises(ValueError, match="Unknown bandit 'nope'"):
                client.pull("nope")
        finally:
            client.close()

    @respx.mock
    def test_pull_with_query(self):
        client = _connected_client()
        try:
            result = client.pull("my-chatbot", query="What is 2+2?")
            assert isinstance(result, PullResult)
        finally:
            client.close()

    @respx.mock
    def test_pull_not_connected_raises(self):
        client = BanditoClient(api_key="x")
        with pytest.raises(RuntimeError, match="Not connected"):
            client.pull("test")

    @respx.mock
    def test_pull_convenience_properties(self):
        client = _connected_client()
        try:
            result = client.pull("my-chatbot")
            # model/prompt should match arm data
            assert result.model == result.arm.model_name
            assert result.prompt == result.arm.system_prompt
        finally:
            client.close()

    @respx.mock
    def test_pull_explore_mode(self):
        """Explore mode bandit should work without errors."""
        d = EXPECTED_DIMS
        chol = [0.0] * (d * d)
        for i in range(d):
            chol[i * d + i] = 1.0
        explore_sync = make_sync_response([{
            "bandit_id": 1, "name": "explore-bot", "type": "online",
            "cost_importance": 0, "latency_importance": 0,
            "optimization_mode": "explore",
            "total_pull_count": 0, "avg_latency_last_n": None,
            "theta": [0.0] * d, "cholesky": chol,
            "dimensions": d,
            "arms": [{**a, "avg_latency_last_n": None} for a in ARM_DATA],
        }])
        client = _connected_client(explore_sync)
        try:
            result = client.pull("explore-bot")
            assert isinstance(result, PullResult)
        finally:
            client.close()

    @respx.mock
    def test_pull_with_latency_context(self):
        """Arms with latency data should compute relative_latency."""
        d = EXPECTED_DIMS
        chol = [0.0] * (d * d)
        for i in range(d):
            chol[i * d + i] = 1.0
        sync = make_sync_response([{
            "bandit_id": 1, "name": "latency-bot", "type": "online",
            "cost_importance": 0, "latency_importance": 3,
            "optimization_mode": "base",
            "total_pull_count": 100, "avg_latency_last_n": 1000.0,
            "theta": [0.0] * d, "cholesky": chol,
            "dimensions": d,
            "arms": [
                {**ARM_DATA[0], "avg_latency_last_n": 800.0},
                {**ARM_DATA[1], "avg_latency_last_n": 1200.0},
                {**ARM_DATA[2], "avg_latency_last_n": 1000.0},
            ],
        }])
        client = _connected_client(sync)
        try:
            result = client.pull("latency-bot")
            assert isinstance(result, PullResult)
        finally:
            client.close()


# ---------- Circuit Breaker (exclude) ----------


class TestExclude:
    """Tests for pull(exclude=...) circuit breaker."""

    @respx.mock
    def test_exclude_single_arm(self):
        """Excluding arm 1 should never return arm 1."""
        client = _connected_client()
        try:
            for _ in range(20):
                result = client.pull("my-chatbot", exclude=[1])
                assert result.arm.arm_id != 1
        finally:
            client.close()

    @respx.mock
    def test_exclude_multiple_arms(self):
        """Excluding arms 1 and 3 should only return arm 2."""
        client = _connected_client()
        try:
            for _ in range(20):
                result = client.pull("my-chatbot", exclude=[1, 3])
                assert result.arm.arm_id == 2
        finally:
            client.close()

    @respx.mock
    def test_exclude_all_raises(self):
        """Excluding all arm IDs raises ValueError."""
        client = _connected_client()
        try:
            with pytest.raises(ValueError, match="All arms excluded"):
                client.pull("my-chatbot", exclude=[1, 2, 3])
        finally:
            client.close()

    @respx.mock
    def test_exclude_empty_list_no_op(self):
        """exclude=[] behaves like no exclusion."""
        client = _connected_client()
        try:
            result = client.pull("my-chatbot", exclude=[])
            assert isinstance(result, PullResult)
        finally:
            client.close()

    @respx.mock
    def test_exclude_nonexistent_id_no_effect(self):
        """Unknown arm_id in exclude list is silently ignored."""
        client = _connected_client()
        try:
            result = client.pull("my-chatbot", exclude=[999])
            assert isinstance(result, PullResult)
            assert result.arm.arm_id in {1, 2, 3}
        finally:
            client.close()

    @respx.mock
    def test_exclude_none_no_op(self):
        """exclude=None (default) behaves normally."""
        client = _connected_client()
        try:
            result = client.pull("my-chatbot", exclude=None)
            assert isinstance(result, PullResult)
        finally:
            client.close()

    @respx.mock
    def test_exclude_scores_omit_excluded_arms(self):
        """Excluded arms should not appear in result.scores."""
        client = _connected_client()
        try:
            result = client.pull("my-chatbot", exclude=[1])
            assert 1 not in result.scores
            assert len(result.scores) == 2
            assert set(result.scores.keys()) <= {2, 3}
        finally:
            client.close()

    @respx.mock
    def test_no_exclude_scores_contain_all_arms(self):
        """Without exclude, scores contains all arms."""
        client = _connected_client()
        try:
            result = client.pull("my-chatbot")
            assert len(result.scores) == 3
            assert set(result.scores.keys()) == {1, 2, 3}
        finally:
            client.close()
