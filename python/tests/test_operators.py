"""Tests for custom operators in flagd_evaluator."""

import pytest


def test_fractional_operator():
    """Test fractional operator for A/B testing."""
    from flagd_evaluator import evaluate_logic

    # Fractional operator should consistently bucket the same user
    result = evaluate_logic(
        {"fractional": [{"var": "userId"}, ["A", 50], ["B", 50]]},
        {"userId": "user123"}
    )
    assert result["success"] is True
    assert result["result"] in ["A", "B"]

    # Same user should get same bucket
    result2 = evaluate_logic(
        {"fractional": [{"var": "userId"}, ["A", 50], ["B", 50]]},
        {"userId": "user123"}
    )
    assert result["result"] == result2["result"]


def test_sem_ver_operator_equals():
    """Test semantic version equals comparison."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": ["=", "1.0.0", "1.0.0"]},
        {}
    )
    assert result["success"] is True
    assert result["result"] is True


def test_sem_ver_operator_greater_than():
    """Test semantic version greater than comparison."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": [">", "2.0.0", "1.0.0"]},
        {}
    )
    assert result["success"] is True
    assert result["result"] is True

    result2 = evaluate_logic(
        {"sem_ver": [">", "1.0.0", "2.0.0"]},
        {}
    )
    assert result2["success"] is True
    assert result2["result"] is False


def test_sem_ver_operator_greater_than_or_equal():
    """Test semantic version >= comparison."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": [">=", "2.0.0", "2.0.0"]},
        {}
    )
    assert result["success"] is True
    assert result["result"] is True


def test_sem_ver_operator_less_than():
    """Test semantic version less than comparison."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": ["<", "1.0.0", "2.0.0"]},
        {}
    )
    assert result["success"] is True
    assert result["result"] is True


def test_sem_ver_operator_caret():
    """Test semantic version caret range."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": ["^", "1.5.0", "1.0.0"]},
        {}
    )
    assert result["success"] is True
    # 1.5.0 should match ^1.0.0 (1.x.x)
    assert result["result"] is True


def test_sem_ver_operator_tilde():
    """Test semantic version tilde range."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"sem_ver": ["~", "1.0.5", "1.0.0"]},
        {}
    )
    assert result["success"] is True
    # 1.0.5 should match ~1.0.0 (1.0.x)
    assert result["result"] is True


def test_starts_with_operator():
    """Test starts_with string operator."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"starts_with": [{"var": "email"}, "admin@"]},
        {"email": "admin@example.com"}
    )
    assert result["success"] is True
    assert result["result"] is True

    result2 = evaluate_logic(
        {"starts_with": [{"var": "email"}, "user@"]},
        {"email": "admin@example.com"}
    )
    assert result2["success"] is True
    assert result2["result"] is False


def test_ends_with_operator():
    """Test ends_with string operator."""
    from flagd_evaluator import evaluate_logic

    result = evaluate_logic(
        {"ends_with": [{"var": "email"}, "@example.com"]},
        {"email": "admin@example.com"}
    )
    assert result["success"] is True
    assert result["result"] is True

    result2 = evaluate_logic(
        {"ends_with": [{"var": "email"}, "@other.com"]},
        {"email": "admin@example.com"}
    )
    assert result2["success"] is True
    assert result2["result"] is False
