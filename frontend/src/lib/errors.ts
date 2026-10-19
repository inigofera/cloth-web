/**
 * Error classification and formatting utilities.
 * Provides contextual error messages with appropriate suggestions based on error type.
 */

export interface ErrorContext {
  message: string;
  type?: 'network' | 'validation' | 'auth' | 'unknown';
  suggestion?: string;
}

/**
 * Classify an error and return a structured context for display.
 */
export function classifyError(err: unknown): ErrorContext {
  const msg = err instanceof Error ? err.message : String(err);

  // If the error represents an HTTP status response, preserve the server's actual error message
  const httpMatch = msg.match(/^(\d{3})\s+([^:]+)(?::\s*([\s\S]*))?$/);
  if (httpMatch) {
    const statusCode = parseInt(httpMatch[1], 10);
    const detail = httpMatch[3]?.trim();
    const serverMsg = detail || msg;

    if (statusCode === 401 || statusCode === 403 || /unauthorized|forbidden|session|token/i.test(serverMsg)) {
      return {
        message: 'Your session has expired. Please sign in again.',
        type: 'auth',
        suggestion: 'Click Sign out and then Sign in.'
      };
    }

    if (statusCode >= 400 && statusCode < 500) {
      return {
        message: serverMsg,
        type: 'validation',
        suggestion: undefined
      };
    }

    return {
      message: serverMsg,
      type: 'unknown',
      suggestion: statusCode === 502 || statusCode === 504 ? 'Please verify your local inference service is running and responsive.' : undefined
    };
  }

  // True browser client fetch failures (e.g. "Failed to fetch", "NetworkError", "net::ERR_CONNECTION_REFUSED")
  if (/fetch|network|timeout|abort/i.test(msg)) {
    return {
      message: 'Network error. Please check your connection and try again.',
      type: 'network',
      suggestion: 'If the problem persists, refresh the page.'
    };
  }

  // Validation errors (from backend)
  if (/validation|invalid|required/i.test(msg)) {
    return {
      message: msg || 'Invalid input. Please check your values.',
      type: 'validation',
      suggestion: undefined
    };
  }

  // Auth errors
  if (/unauthorized|forbidden|session|token/i.test(msg)) {
    return {
      message: 'Your session has expired. Please sign in again.',
      type: 'auth',
      suggestion: 'Click Sign out and then Sign in.'
    };
  }

  // Unknown errors - show raw but wrapped nicely
  return {
    message: msg || 'An unexpected error occurred.',
    type: 'unknown'
  };
}

/**
 * Format an ErrorContext for display to the user.
 */
export function formatError(context?: ErrorContext): string {
  if (!context) return '';
  let result = context.message;

  if (context.suggestion) {
    result += `\n\n${context.suggestion}`;
  }

  return result;
}

/**
 * Check if an error is a network-related error.
 */
export function isNetworkError(err: string): boolean {
  return /fetch|network|timeout|abort/i.test(err);
}