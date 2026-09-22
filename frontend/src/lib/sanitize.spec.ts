import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  sanitizeText,
  sanitizeOption,
  sanitizeInt,
  sanitizeDecimal,
  isValidHexColor,
  isValidDate,
  isValidEmail,
  isValidObjectPath,
  hasImageMagicBytes,
  validateImageFile,
  MAX_UPLOAD_BYTES,
  MAX_IMAGE_DIMENSION,
} from './sanitize';

const ORIGINS = ['Bought New', '2nd Hand', 'Gift', 'Borrowed', 'Made Myself'] as const;
const IMPACTS = ['Nothing', 'Low', 'Medium', 'High'] as const;

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

describe('sanitizeText', () => {
  it('trims and caps length', () => {
    expect(sanitizeText('  hello  ', { max: 100 })).toBe('hello');
    expect(sanitizeText('a'.repeat(500), { max: 100 })).toHaveLength(100);
  });

  it('strips control characters by default', () => {
    expect(sanitizeText('a\u0000b\u001Fc', { max: 100 })).toBe('abc');
  });

  it('keeps newlines when requested', () => {
    expect(sanitizeText('a\nb\u0000c', { max: 100, keepNewlines: true })).toBe('a\nbc');
  });

  it('returns empty for non-strings', () => {
    expect(sanitizeText(null, { max: 10 })).toBe('');
    expect(sanitizeText(42, { max: 10 })).toBe('');
  });
});

describe('sanitizeOption', () => {
  it('accepts only canonical values', () => {
    expect(sanitizeOption('Gift', ORIGINS)).toBe('Gift');
    expect(sanitizeOption('High', IMPACTS)).toBe('High');
  });

  it('maps blank and unknown values to null', () => {
    expect(sanitizeOption('', ORIGINS)).toBeNull();
    expect(sanitizeOption('Spain', ORIGINS)).toBeNull();
    expect(sanitizeOption('high', IMPACTS)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

describe('sanitizeInt', () => {
  it('parses and validates range', () => {
    expect(sanitizeInt('5', { min: 0, max: 100 })).toBe(5);
    expect(sanitizeInt(5, { min: 0, max: 100 })).toBe(5);
  });

  it('rejects empty, non-numeric, non-integral, and out-of-range', () => {
    expect(sanitizeInt('', { min: 0, max: 100 })).toBeNull();
    expect(sanitizeInt(null, { min: 0, max: 100 })).toBeNull();
    expect(sanitizeInt('abc', { min: 0, max: 100 })).toBeNull();
    expect(sanitizeInt(3.5, { min: 0, max: 100 })).toBeNull();
    expect(sanitizeInt(-1, { min: 0, max: 100 })).toBeNull();
    expect(sanitizeInt(101, { min: 0, max: 100 })).toBeNull();
  });
});

describe('sanitizeDecimal', () => {
  it('rounds to dp and validates range', () => {
    expect(sanitizeDecimal('10.555', { min: 0, max: 1000, dp: 2 })).toBe(10.56);
    expect(sanitizeDecimal(0, { min: 0, max: 1000, dp: 2 })).toBe(0);
  });

  it('rejects invalid and out-of-range', () => {
    expect(sanitizeDecimal('', { min: 0, max: 1000, dp: 2 })).toBeNull();
    expect(sanitizeDecimal('abc', { min: 0, max: 1000, dp: 2 })).toBeNull();
    expect(sanitizeDecimal(-1, { min: 0, max: 1000, dp: 2 })).toBeNull();
    expect(sanitizeDecimal(1001, { min: 0, max: 1000, dp: 2 })).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Format validators
// ---------------------------------------------------------------------------

describe('isValidHexColor', () => {
  it('accepts #RRGGBB', () => {
    expect(isValidHexColor('#a1b2c3')).toBe(true);
    expect(isValidHexColor('  #A1B2C3  ')).toBe(true);
  });

  it('rejects invalid values', () => {
    expect(isValidHexColor('a1b2c3')).toBe(false);
    expect(isValidHexColor('#abc')).toBe(false);
    expect(isValidHexColor('#gggggg')).toBe(false);
    expect(isValidHexColor('')).toBe(false);
    expect(isValidHexColor(null)).toBe(false);
  });
});

describe('isValidDate', () => {
  it('accepts valid dates', () => {
    expect(isValidDate('2020-05-17')).toBe(true);
    expect(isValidDate('1900-01-01')).toBe(true);
  });

  it('rejects impossible, too-old, malformed, and future dates', () => {
    expect(isValidDate('2020-13-01')).toBe(false);
    expect(isValidDate('2023-02-30')).toBe(false);
    expect(isValidDate('1899-12-31')).toBe(false);
    expect(isValidDate('not-a-date')).toBe(false);
    expect(isValidDate('')).toBe(false);
    expect(isValidDate('2999-01-01')).toBe(false);
  });
});

describe('isValidEmail', () => {
  it('accepts valid emails', () => {
    expect(isValidEmail('a@b.co')).toBe(true);
    expect(isValidEmail('  user@example.com  ')).toBe(true);
  });

  it('rejects invalid emails', () => {
    expect(isValidEmail('nope')).toBe(false);
    expect(isValidEmail('a@b')).toBe(false);
    expect(isValidEmail('')).toBe(false);
    expect(isValidEmail('a b@c.d')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Image object path (shape only — ownership is enforced by the backend)
// ---------------------------------------------------------------------------

const USER = '11111111-1111-4111-8111-111111111111';
const OBJ = '22222222-2222-4222-8222-222222222222';

describe('isValidObjectPath', () => {
  it('accepts a well-formed path', () => {
    expect(isValidObjectPath(`users/${USER}/${OBJ}-photo.jpg`)).toBe(true);
    expect(isValidObjectPath(`users/${USER}/${OBJ}-a_b-c.png`)).toBe(true);
  });

  it('rejects traversal, injection, missing/bad extension, and external URLs', () => {
    expect(isValidObjectPath(`users/${USER}/../../etc/passwd.png`)).toBe(false);
    expect(isValidObjectPath(`users/${USER}/${OBJ}-photo.jpg?width=9`)).toBe(false);
    expect(isValidObjectPath(`users/${USER}/${OBJ}-photo.svg`)).toBe(false);
    expect(isValidObjectPath(`users/${USER}/${OBJ}-photo`)).toBe(false);
    expect(isValidObjectPath('https://evil.com/x.png')).toBe(false);
    expect(isValidObjectPath('')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Magic bytes
// ---------------------------------------------------------------------------

const JPEG_HEAD = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
const PNG_HEAD = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x1a, 0x0a, 0x0a, 0, 0, 0, 0, 0, 0, 0, 0]);
const GIF_HEAD = new Uint8Array([0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
const WEBP_HEAD = new Uint8Array([0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0, 0, 0, 0]);
const HTML_BYTES = new Uint8Array([0x3c, 0x68, 0x74, 0x6d, 0x6c, 0x3e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

describe('hasImageMagicBytes', () => {
  it('accepts valid signatures', () => {
    expect(hasImageMagicBytes(JPEG_HEAD)).toBe(true);
    expect(hasImageMagicBytes(PNG_HEAD)).toBe(true);
    expect(hasImageMagicBytes(GIF_HEAD)).toBe(true);
    expect(hasImageMagicBytes(WEBP_HEAD)).toBe(true);
  });

  it('rejects non-image or too-short bytes', () => {
    expect(hasImageMagicBytes(HTML_BYTES)).toBe(false);
    expect(hasImageMagicBytes(new Uint8Array(4))).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Image file validation
// ---------------------------------------------------------------------------

describe('validateImageFile', () => {
  const mockBitmap = (w: number, h: number) => ({ width: w, height: h, close: vi.fn() });

  beforeEach(() => {
    vi.stubGlobal('createImageBitmap', vi.fn(async () => mockBitmap(100, 100)));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('accepts a valid small JPEG', async () => {
    const file = new File([JPEG_HEAD], 'photo.jpg', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(true);
    expect(res.file).toBe(file);
  });

  it('rejects a disallowed MIME type', async () => {
    const file = new File([JPEG_HEAD], 'photo.svg', { type: 'image/svg+xml' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/type/i);
  });

  it('rejects a disallowed extension', async () => {
    const file = new File([JPEG_HEAD], 'photo.bmp', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/extension/i);
  });

  it('rejects an empty file', async () => {
    const file = new File([new Uint8Array(0)], 'photo.jpg', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/empty/i);
  });

  it('rejects an oversized file', async () => {
    const big = new Uint8Array(MAX_UPLOAD_BYTES + 1);
    big[0] = 0xff;
    big[1] = 0xd8;
    big[2] = 0xff;
    const file = new File([big], 'big.jpg', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/too large/i);
  });

  it('rejects content that is not a real image', async () => {
    const file = new File([HTML_BYTES], 'photo.jpg', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/content/i);
  });

  it('rejects an image with oversized dimensions', async () => {
    vi.stubGlobal('createImageBitmap', vi.fn(async () => mockBitmap(MAX_IMAGE_DIMENSION + 1, 100)));
    const file = new File([JPEG_HEAD], 'photo.jpg', { type: 'image/jpeg' });
    const res = await validateImageFile(file);
    expect(res.ok).toBe(false);
    expect(res.error).toMatch(/too large/i);
  });
});
