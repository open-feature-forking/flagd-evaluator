"""Type stubs for flagd_evaluator module."""

from typing import Any, Dict, Optional, TypedDict


class EvaluationResult(TypedDict):
    """Result from flag evaluation."""
    value: Any
    variant: Optional[str]
    reason: str
    errorCode: Optional[str]
    errorMessage: Optional[str]
    flagMetadata: Dict[str, Any]


class FlagEvaluator:
    """
    Stateful feature flag evaluator.

    This class maintains an internal state of feature flag configurations
    and provides methods to evaluate flags against context data.

    Example:
        >>> evaluator = FlagEvaluator()
        >>> evaluator.update_state({
        ...     "flags": {
        ...         "myFlag": {
        ...             "state": "ENABLED",
        ...             "variants": {"on": True, "off": False},
        ...             "defaultVariant": "on"
        ...         }
        ...     }
        ... })
        >>> result = evaluator.evaluate_bool("myFlag", {}, False)
        >>> print(result)
        True
    """

    def __init__(self) -> None:
        """Create a new FlagEvaluator instance."""
        ...

    def update_state(self, config: Dict[str, Any]) -> Dict[str, bool]:
        """
        Update the flag configuration state.

        Args:
            config: Flag configuration in flagd format

        Returns:
            Update response with success status

        Raises:
            ValueError: If configuration is invalid
        """
        ...

    def evaluate(self, flag_key: str, context: Dict[str, Any]) -> EvaluationResult:
        """
        Evaluate a feature flag.

        Args:
            flag_key: The flag key to evaluate
            context: Evaluation context

        Returns:
            Evaluation result with value, variant, reason, and metadata

        Raises:
            RuntimeError: If no state is loaded
            KeyError: If flag is not found
        """
        ...

    def evaluate_bool(
        self,
        flag_key: str,
        context: Dict[str, Any],
        default_value: bool
    ) -> bool:
        """
        Evaluate a boolean flag.

        Args:
            flag_key: The flag key to evaluate
            context: Evaluation context
            default_value: Default value if evaluation fails

        Returns:
            The evaluated boolean value

        Raises:
            RuntimeError: If no state is loaded
            KeyError: If flag is not found
        """
        ...

    def evaluate_string(
        self,
        flag_key: str,
        context: Dict[str, Any],
        default_value: str
    ) -> str:
        """
        Evaluate a string flag.

        Args:
            flag_key: The flag key to evaluate
            context: Evaluation context
            default_value: Default value if evaluation fails

        Returns:
            The evaluated string value

        Raises:
            RuntimeError: If no state is loaded
            KeyError: If flag is not found
        """
        ...

    def evaluate_int(
        self,
        flag_key: str,
        context: Dict[str, Any],
        default_value: int
    ) -> int:
        """
        Evaluate an integer flag.

        Args:
            flag_key: The flag key to evaluate
            context: Evaluation context
            default_value: Default value if evaluation fails

        Returns:
            The evaluated integer value

        Raises:
            RuntimeError: If no state is loaded
            KeyError: If flag is not found
        """
        ...

    def evaluate_float(
        self,
        flag_key: str,
        context: Dict[str, Any],
        default_value: float
    ) -> float:
        """
        Evaluate a float flag.

        Args:
            flag_key: The flag key to evaluate
            context: Evaluation context
            default_value: Default value if evaluation fails

        Returns:
            The evaluated float value

        Raises:
            RuntimeError: If no state is loaded
            KeyError: If flag is not found
        """
        ...
