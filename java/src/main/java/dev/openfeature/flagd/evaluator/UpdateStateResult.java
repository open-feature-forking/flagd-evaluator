package dev.openfeature.flagd.evaluator;

import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.List;

/**
 * Result of updating flag state.
 *
 * <p>Contains success status, optional error information, and a list of changed flag keys.
 */
public class UpdateStateResult {

    private boolean success;
    private String error;

    private List<String> changedFlags;

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

    @Override
    public String toString() {
        return "UpdateStateResult{" +
                "success=" + success +
                (error != null ? ", error='" + error + '\'' : "") +
                (changedFlags != null ? ", changedFlags=" + changedFlags : "") +
                '}';
    }
}
