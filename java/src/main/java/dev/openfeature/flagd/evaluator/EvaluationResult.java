package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.annotation.JsonProperty;
import dev.openfeature.sdk.ImmutableMetadata;

import java.util.Map;

/**
 * Result of a flag evaluation.
 *
 * <p>Contains the resolved value, variant, reason, and optional error information.
 */
public class EvaluationResult<T> {

    private T value;
    private String variant;
    private String reason;

    private String errorCode;

    private String errorMessage;

    private ImmutableMetadata flagMetadata;

    public EvaluationResult() {
    }

    /**
     * Gets the resolved flag value.
     *
     * @return the value (can be Boolean, String, Number, or Map)
     */
    public T getValue() {
        return value;
    }

    public void setValue(T value) {
        this.value = value;
    }

    /**
     * Gets the variant that was selected.
     *
     * @return the variant name, or null if not applicable
     */
    public String getVariant() {
        return variant;
    }

    public void setVariant(String variant) {
        this.variant = variant;
    }

    /**
     * Gets the resolution reason.
     *
     * <p>Possible values:
     * <ul>
     *   <li>STATIC - Resolved to the default variant
     *   <li>TARGETING_MATCH - Resolved via targeting rules
     *   <li>DISABLED - Flag is disabled
     *   <li>ERROR - An error occurred
     *   <li>FLAG_NOT_FOUND - Flag key not found
     * </ul>
     *
     * @return the resolution reason
     */
    public String getReason() {
        return reason;
    }

    public void setReason(String reason) {
        this.reason = reason;
    }

    /**
     * Gets the error code if an error occurred.
     *
     * @return the error code, or null if no error
     */
    public String getErrorCode() {
        return errorCode;
    }

    public void setErrorCode(String errorCode) {
        this.errorCode = errorCode;
    }

    /**
     * Gets the error message if an error occurred.
     *
     * @return the error message, or null if no error
     */
    public String getErrorMessage() {
        return errorMessage;
    }

    public void setErrorMessage(String errorMessage) {
        this.errorMessage = errorMessage;
    }

    /**
     * Gets the flag metadata.
     *
     * @return the metadata map, or null if no metadata
     */
    public ImmutableMetadata getFlagMetadata() {
        return flagMetadata;
    }

    public void setFlagMetadata(ImmutableMetadata flagMetadata) {
        this.flagMetadata = flagMetadata;
    }

    /**
     * Checks if this evaluation resulted in an error.
     *
     * @return true if an error occurred
     */
    public boolean isError() {
        return errorCode != null;
    }

    @Override
    public String toString() {
        return "EvaluationResult{" +
                "value=" + value +
                ", variant='" + variant + '\'' +
                ", reason='" + reason + '\'' +
                (errorCode != null ? ", errorCode='" + errorCode + '\'' : "") +
                (errorMessage != null ? ", errorMessage='" + errorMessage + '\'' : "") +
                '}';
    }
}
