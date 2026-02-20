namespace FlagdEvaluator;

/// <summary>
/// Exception thrown by the flag evaluator for WASM or evaluation errors.
/// </summary>
public class EvaluatorException : Exception
{
    public EvaluatorException(string message) : base(message) { }
    public EvaluatorException(string message, Exception innerException) : base(message, innerException) { }
}
