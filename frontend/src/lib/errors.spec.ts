import { describe, it, expect } from 'vitest';
import { classifyError, formatError, isNetworkError } from './errors';

describe('classifyError', () => {
  it('identifies network errors', () => {
    const err = new Error('Failed to fetch');
    const res = classifyError(err);
    expect(res.type).toBe('network');
    expect(res.message).toContain('Network error');
    expect(res.suggestion).toBeDefined();
  });

  it('identifies validation errors', () => {
    const err = new Error('Validation failed: name is required');
    const res = classifyError(err);
    expect(res.type).toBe('validation');
    expect(res.message).toContain('Validation failed');
  });

  it('identifies auth errors', () => {
    const err = new Error('401 unauthorized: ExpiredSignature');
    const res = classifyError(err);
    expect(res.type).toBe('auth');
    expect(res.message).toContain('session has expired');
  });

  it('handles unknown errors safely', () => {
    const res = classifyError('Something completely random');
    expect(res.type).toBe('unknown');
    expect(res.message).toBe('Something completely random');
  });

  it('handles non-string non-error values', () => {
    const res = classifyError({ foo: 'bar' });
    expect(res.type).toBe('unknown');
    expect(res.message).toBe('[object Object]');
  });
});

describe('formatError', () => {
  it('formats context with suggestion', () => {
    const formatted = formatError({
      message: 'Failed to load',
      suggestion: 'Please try again later'
    });
    expect(formatted).toBe('Failed to load\n\nPlease try again later');
  });

  it('formats context without suggestion', () => {
    expect(formatError({ message: 'Error only' })).toBe('Error only');
  });

  it('handles undefined context', () => {
    expect(formatError(undefined)).toBe('');
  });
});

describe('isNetworkError', () => {
  it('detects network keywords', () => {
    expect(isNetworkError('network timeout')).toBe(true);
    expect(isNetworkError('abort error')).toBe(true);
    expect(isNetworkError('syntax error')).toBe(false);
  });
});
