package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;
import java.util.Map;

/**
 * Result of updating flag state.
 *
 * <p>Contains success status, optional error information, and a list of changed flag keys.
 */
public class UpdateStateResult {

    private boolean success;
    private String error;

    private List<String> changedFlags;

    private Map<String, EvaluationResult<Object>> preEvaluated;

    private Map<String, java.util.List<String>> requiredContextKeys;

    private Map<String, Integer> flagIndices;

    public UpdateStateResult() {
    }

    /**
     * Checks if the update was successful.
     *
     * @return true if successful, false if validation or parsing failed
     */
    public boolean isSuccess() {
        return success;
    }

    public void setSuccess(boolean success) {
        this.success = success;
    }

    /**
     * Gets the error message if the update failed.
     *
     * @return the error message, or null if successful
     */
    public String getError() {
        return error;
    }

    public void setError(String error) {
        this.error = error;
    }

    /**
     * Gets the list of changed flag keys.
     *
     * <p>This includes flags that were added, modified, or removed.
     *
     * @return the list of changed flag keys, or null if the update failed
     */
    public List<String> getChangedFlags() {
        return changedFlags;
    }

    public void setChangedFlags(List<String> changedFlags) {
        this.changedFlags = changedFlags;
    }

    /**
     * Gets the pre-evaluated results for static and disabled flags.
     *
     * <p>These flags don't require targeting evaluation, so their results are
     * computed during {@code updateState()} to allow host-side caching.
     *
     * @return map of flag key to pre-evaluated result, or null if none
     */
    public Map<String, EvaluationResult<Object>> getPreEvaluated() {
        return preEvaluated;
    }

    public void setPreEvaluated(Map<String, EvaluationResult<Object>> preEvaluated) {
        this.preEvaluated = preEvaluated;
    }

    /**
     * Gets the required context keys per flag for host-side filtering.
     *
     * <p>When present for a flag, the host should only serialize these context keys
     * (plus {@code $flagd.*} enrichment and {@code targetingKey}) before calling evaluate.
     * If a flag is absent from this map, send the full context.
     *
     * @return map of flag key to required context keys, or null if not available
     */
    public Map<String, java.util.List<String>> getRequiredContextKeys() {
        return requiredContextKeys;
    }

    public void setRequiredContextKeys(Map<String, java.util.List<String>> requiredContextKeys) {
        this.requiredContextKeys = requiredContextKeys;
    }

    /**
     * Gets the flag key to numeric index mapping for {@code evaluate_by_index}.
     *
     * <p>Allows calling the WASM {@code evaluate_by_index(index, ...)} export
     * instead of passing flag key strings.
     *
     * @return map of flag key to numeric index, or null if not available
     */
    public Map<String, Integer> getFlagIndices() {
        return flagIndices;
    }

    public void setFlagIndices(Map<String, Integer> flagIndices) {
        this.flagIndices = flagIndices;
    }

    @Override
    public String toString() {
        return "UpdateStateResult{" +
                "success=" + success +
                (error != null ? ", error='" + error + '\'' : "") +
                (changedFlags != null ? ", changedFlags=" + changedFlags : "") +
                '}';
    }
}
