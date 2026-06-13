/**
 * `Result`, the typed `ApiError`, and the runtime-validation helpers that
 * live at the `fetch` boundary.
 *
 * Error-handling split (typescript.md § 6): *expected* domain failures — a
 * 4xx the caller may want to branch on — are returned as `Result<T, ApiError>`
 * values; *unexpected* failures (network down, 5xx, a 2xx body that fails its
 * Zod schema) throw and bubble to an error boundary. `ApiErrorException` is
 * the thrown carrier — it subclasses `Error` (so existing `catch (e)` /
 * `e.message` sites keep working) while also exposing the structured
 * `apiError` for catch sites that want to branch on `.code`.
 *
 * PR-B2 (Issue #191).
 */
import * as z from 'zod';

/**
 * Expected-failure-as-value. A helper returns `Result` only when its caller
 * is expected to handle the failure inline; everything else throws.
 */
export type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

/**
 * Stable error `code` values. The first block mirrors the wire registry in
 * api-contract.md § 7 one-for-one — renaming a code there is a breaking wire
 * change. The last two are client-synthesized:
 *  - `response_shape_mismatch` — a response parsed as JSON but failed its Zod
 *    schema (§ 8: contract drift, treated as a programmer error).
 *  - `unknown` — the backend sent a `code` we don't recognize, or none at all.
 */
export const API_ERROR_CODES = [
  'bad_request',
  'lap_times_mismatch',
  'track_id_mismatch',
  'invalid_path_param',
  'invalid_request_body',
  'invalid_credentials',
  'token_expired',
  'token_invalid',
  'token_reuse_detected',
  'forbidden',
  'admin_required',
  'not_found',
  'username_taken',
  'session_closed',
  'pending_races_first',
  'out_of_order_submission',
  'race_number_conflict',
  'unprocessable',
  'internal',
  'gateway_timeout',
  'response_shape_mismatch',
  'unknown',
] as const;

export type ApiErrorCode = (typeof API_ERROR_CODES)[number];

/**
 * The frontend's view of the backend error envelope (`{ error, code }`,
 * api-contract.md § 2). It discriminates on `code`: a `switch (err.code)`
 * narrows exhaustively against the closed `ApiErrorCode` set.
 *
 * Modeled as one shape rather than an expanded `{ code: 'a' } | { code: 'b' }`
 * union because no code carries a code-specific payload today — every member
 * would be the identical `{ code; message }`. An expanded union also can't be
 * *constructed* from a dynamically-parsed `code` without an unchecked cast.
 * If a future code grows extra fields, split `ApiError` into a real per-code
 * union at that point.
 */
export type ApiError = {
  code: ApiErrorCode;
  message: string;
};

/** Thrown carrier for an `ApiError` — see the file header. */
export class ApiErrorException extends Error {
  readonly apiError: ApiError;

  constructor(apiError: ApiError, options?: ErrorOptions) {
    super(apiError.message, options);
    this.name = 'ApiErrorException';
    this.apiError = apiError;
  }
}

function isApiErrorCode(code: string): code is ApiErrorCode {
  // The `as readonly string[]` only widens the literal tuple so `.includes`
  // accepts an arbitrary string — it is not an unsafe narrowing cast.
  return (API_ERROR_CODES as readonly string[]).includes(code);
}

/**
 * The wire error envelope. `code` is optional: § 2 says the backend always
 * sends it, but a malformed or proxy-generated error response might not, and
 * a missing code degrades to `unknown` rather than throwing.
 */
const errorEnvelopeSchema = z.object({
  error: z.string(),
  code: z.string().optional(),
});

function httpStatusError(status: number): ApiError {
  return {
    code: 'unknown',
    message: `Request failed (HTTP ${String(status)})`,
  };
}

/**
 * Read a non-2xx `Response` into a typed `ApiError`. Never throws: a body
 * that isn't the expected envelope (empty, non-JSON, wrong shape) degrades to
 * an `unknown`-coded error carrying an HTTP-status message.
 */
export async function parseApiError(res: Response): Promise<ApiError> {
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    return httpStatusError(res.status);
  }
  const parsed = errorEnvelopeSchema.safeParse(body);
  if (!parsed.success) return httpStatusError(res.status);
  const { error, code } = parsed.data;
  return {
    code: code !== undefined && isApiErrorCode(code) ? code : 'unknown',
    message: error,
  };
}

/**
 * Parse a 2xx `Response` body through a Zod schema. On a JSON or schema
 * failure throws `ApiErrorException` with `code: 'response_shape_mismatch'`
 * (§ 8: contract drift fails loud) — the underlying error is preserved as
 * `cause` so the original stack stays reachable.
 */
export async function parseBody<S extends z.ZodType>(
  schema: S,
  res: Response,
): Promise<z.output<S>> {
  let body: unknown;
  try {
    body = await res.json();
  } catch (cause) {
    throw new ApiErrorException(
      {
        code: 'response_shape_mismatch',
        message: 'Response body was not valid JSON',
      },
      { cause },
    );
  }
  const parsed = schema.safeParse(body);
  if (!parsed.success) {
    throw new ApiErrorException(
      {
        code: 'response_shape_mismatch',
        message: `Response failed schema validation: ${parsed.error.message}`,
      },
      { cause: parsed.error },
    );
  }
  return parsed.data;
}

/**
 * Log a swallowed error if — and only if — it's a `response_shape_mismatch`.
 *
 * The legacy `useEffect`-fetch hooks intentionally degrade to an empty result
 * on a network failure, but § 8 says contract drift must stay visible. This
 * lets those catch blocks keep their network-error behavior while surfacing a
 * `parseBody` schema failure to the console. PR-C1 retired the per-hook call
 * sites by moving the read hooks to TanStack Query; the cache's `onError`
 * (see `api/queryClient.ts`) now routes every query error through this same
 * helper, so drift stays loud while network errors stay quiet. Still called
 * directly by `client.ts` for the silent auth-refresh path.
 */
export function logIfResponseShapeMismatch(
  error: unknown,
  context: string,
): void {
  if (
    error instanceof ApiErrorException &&
    error.apiError.code === 'response_shape_mismatch'
  ) {
    console.error(`Response shape mismatch: ${context}`, error);
  }
}
