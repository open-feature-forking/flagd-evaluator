package dev.openfeature.flagd.evaluator;

/**
 * Exception thrown when flag evaluation fails.
 *
 * <p>This exception wraps underlying errors that may occur during:
 * <ul>
 *   <li>WASM module interaction
 *   <li>JSON serialization/deserialization
 *   <li>Memory allocation/deallocation
 *   <li>Flag configuration parsing
 * </ul>
 */
public class EvaluatorException extends Exception {

    /**
     * Creates a new evaluator exception with the specified message.
     *
     * @param message the error message
     */
    public EvaluatorException(String message) {
        super(message);
    }

    /**
     * Creates a new evaluator exception with the specified message and cause.
     *
     * @param message the error message
     * @param cause the underlying cause
     */
    public EvaluatorException(String message, Throwable cause) {
        super(message, cause);
    }
}
