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

    @Override
    public String toString() {
        return "UpdateStateResult{" +
                "success=" + success +
                (error != null ? ", error='" + error + '\'' : "") +
                (changedFlags != null ? ", changedFlags=" + changedFlags : "") +
                '}';
    }
}
