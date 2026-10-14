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

  // Network errors (4xx/5xx, fetch failures)
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