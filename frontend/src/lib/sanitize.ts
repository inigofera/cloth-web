/**
 * Client-side input sanitization.
 *
 * These helpers mirror the backend's rules (see `src/upload.rs` and
 * `src/routes.rs`) so that invalid input is rejected before it is uploaded or
 * sent to the API — faster feedback and less wasted bandwidth. The backend
 * remains the source of truth; this is defense in depth.
 *
 * All functions are pure (except `validateImageFile`, which reads the file)
 * and unit-testable.
 */

// ---------------------------------------------------------------------------
// Limits (kept in sync with the backend)
// ---------------------------------------------------------------------------

/** Maximum accepted upload size (10 MiB) — matches `MAX_UPLOAD_BYTES`. */
export const MAX_UPLOAD_BYTES = 10 * 1024 * 1024;

/** Allowed image MIME types — matches the backend extension allowlist. */
export const ALLOWED_IMAGE_TYPES: readonly string[] = [
  'image/jpeg',
  'image/png',
  'image/webp',
  'image/gif',
];

/** Allowed image extensions (lowercase, no leading dot). */
export const ALLOWED_IMAGE_EXT: readonly string[] = ['jpg', 'jpeg', 'png', 'webp', 'gif'];

/** Client-side decode guard: reject images larger than this on any side. */
export const MAX_IMAGE_DIMENSION = 8000;

// Field length / range limits.
export const NAME_MAX = 100;
export const NOTES_MAX = 2000;
export const OUTFIT_NOTES_MAX = 500;
export const ORIGIN_MAX = 100;
export const LAUNDRY_MAX = 50;
export const COLOR_NAME_MAX = 50;
export const PRICE_MAX = 1_000_000;
export const WEAR_MAX = 100_000;
export const EMAIL_MAX = 254;
export const PASSWORD_MAX = 100;

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

const CTRL_ALL = /[\u0000-\u001F\u007F]/g;
const CTRL_KEEP_NEWLINES = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g;

export interface SanitizeTextOptions {
  max: number;
  /** Preserve tabs, newlines and carriage returns (for multi-line notes). */
  keepNewlines?: boolean;
}

/**
 * Trim a string, strip control characters, and cap its length.
 * Returns `''` for empty/invalid input.
 */
export function sanitizeText(value: unknown, { max, keepNewlines = false }: SanitizeTextOptions): string {
  if (typeof value !== 'string') return '';
  const cleaned = value.replace(keepNewlines ? CTRL_KEEP_NEWLINES : CTRL_ALL, '');
  return cleaned.trim().slice(0, max);
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

export interface SanitizeIntOptions {
  min: number;
  max: number;
}

/**
 * Coerce to a finite integer within `[min, max]`. Returns `null` when the
 * input is empty, non-numeric, non-integral, or out of range.
 */
export function sanitizeInt(value: unknown, { min, max }: SanitizeIntOptions): number | null {
  if (value === null || value === undefined || value === '') return null;
  const n = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(n) || !Number.isInteger(n)) return null;
  if (n < min || n > max) return null;
  return n;
}

export interface SanitizeDecimalOptions {
  min: number;
  max: number;
  /** Number of decimal places to round to. */
  dp: number;
}

/**
 * Coerce to a finite number within `[min, max]`, rounded to `dp` decimals.
 * Returns `null` when the input is empty, non-numeric, or out of range.
 */
export function sanitizeDecimal(value: unknown, { min, max, dp }: SanitizeDecimalOptions): number | null {
  if (value === null || value === undefined || value === '') return null;
  const n = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(n)) return null;
  const factor = 10 ** dp;
  const rounded = Math.round(n * factor) / factor;
  if (rounded < min || rounded > max) return null;
  return rounded;
}

// ---------------------------------------------------------------------------
// Format validators
// ---------------------------------------------------------------------------

const HEX_RE = /^#[0-9a-fA-F]{6}$/;

/** True when the value is a `#RRGGBB` hex color. */
export function isValidHexColor(value: unknown): boolean {
  return typeof value === 'string' && HEX_RE.test(value.trim());
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * True when the value is a real calendar date in `YYYY-MM-DD` form, between
 * 1900-01-01 and today (inclusive).
 */
export function isValidDate(value: unknown): boolean {
  if (typeof value !== 'string' || !DATE_RE.test(value)) return false;
  const [y, m, d] = value.split('-').map(Number);
  const dt = new Date(Date.UTC(y, m - 1, d));
  if (dt.getUTCFullYear() !== y || dt.getUTCMonth() !== m - 1 || dt.getUTCDate() !== d) return false;
  if (y < 1900) return false;
  const today = new Date();
  const todayUtc = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate());
  return dt.getTime() <= todayUtc;
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/** Basic structural email check plus a length cap. */
export function isValidEmail(value: unknown): boolean {
  if (typeof value !== 'string') return false;
  const v = value.trim();
  return v.length > 0 && v.length <= EMAIL_MAX && EMAIL_RE.test(v);
}

// ---------------------------------------------------------------------------
// Image object path
// ---------------------------------------------------------------------------

const UUID = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}';

/**
 * Shape of a storage object path produced by the backend:
 * `users/{user_id}/{uuid}-{sanitized_name}.{ext}`.
 */
const OBJECT_PATH_RE = new RegExp(
  `^users/${UUID}/${UUID}-[A-Za-z0-9-_]+\\.(jpg|jpeg|png|webp|gif)$`,
);

/**
 * True when the value is a well-formed, single-user object path. Rejects
 * traversal (`..`), query/fragment injection, foreign users' prefixes, and
 * any non-image extension.
 */
export function isValidObjectPath(value: unknown): boolean {
  return typeof value === 'string' && OBJECT_PATH_RE.test(value);
}

// ---------------------------------------------------------------------------
// Image file validation
// ---------------------------------------------------------------------------

export interface ImageValidationResult {
  ok: boolean;
  error?: string;
  file: File | null;
}

/**
 * True when the leading bytes match a JPEG, PNG, GIF, or WEBP signature.
 * Mirrors the backend's magic-byte sniffing.
 */
export function hasImageMagicBytes(bytes: Uint8Array): boolean {
  if (bytes.length < 12) return false;
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return true; // JPEG
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return true; // PNG
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x38) return true; // GIF
  if (
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50
  ) {
    return true; // WEBP
  }
  return false;
}

async function readImageDimensions(
  file: File,
): Promise<{ width: number; height: number } | null> {
  if (typeof createImageBitmap === 'function') {
    try {
      const bmp = await createImageBitmap(file);
      const { width, height } = bmp;
      bmp.close();
      return { width, height };
    } catch {
      // fall through to the <img> path
    }
  }
  return new Promise(resolve => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const { naturalWidth, naturalHeight } = img;
      URL.revokeObjectURL(url);
      resolve({ width: naturalWidth, height: naturalHeight });
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      resolve(null);
    };
    img.src = url;
  });
}

/**
 * Validate an uploaded image file before it is sent to the backend:
 * MIME type, extension, size, magic bytes, and decoded dimensions.
 * Returns the original file on success, or a descriptive error on failure.
 */
export async function validateImageFile(file: File): Promise<ImageValidationResult> {
  if (!ALLOWED_IMAGE_TYPES.includes(file.type)) {
    return { ok: false, error: 'Unsupported file type. Use JPG, PNG, WebP, or GIF.', file: null };
  }

  const ext = file.name.split('.').pop()?.toLowerCase() ?? '';
  if (!ALLOWED_IMAGE_EXT.includes(ext)) {
    return {
      ok: false,
      error: 'Unsupported file extension. Use .jpg, .jpeg, .png, .webp, or .gif.',
      file: null,
    };
  }

  if (file.size === 0) {
    return { ok: false, error: 'File is empty.', file: null };
  }
  if (file.size > MAX_UPLOAD_BYTES) {
    return {
      ok: false,
      error: `File is too large (${(file.size / 1024 / 1024).toFixed(1)} MiB). Max is 10 MiB.`,
      file: null,
    };
  }

  const head = new Uint8Array(await file.slice(0, 16).arrayBuffer());
  if (!hasImageMagicBytes(head)) {
    return { ok: false, error: 'File content does not match a valid image.', file: null };
  }

  const dims = await readImageDimensions(file);
  if (!dims) {
    return { ok: false, error: 'Could not read image dimensions.', file: null };
  }
  if (dims.width <= 0 || dims.height <= 0) {
    return { ok: false, error: 'Image has invalid dimensions.', file: null };
  }
  if (Math.max(dims.width, dims.height) > MAX_IMAGE_DIMENSION) {
    return {
      ok: false,
      error: `Image is too large (${dims.width}\u00d7${dims.height}). Max side is ${MAX_IMAGE_DIMENSION}px.`,
      file: null,
    };
  }

  return { ok: true, file };
}
