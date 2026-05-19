import { describe, expect, it } from 'vitest';
import * as z from 'zod';
import { ZodError } from 'zod';
import { ApiErrorException, parseApiError, parseBody } from './result';

// Verifies the fetch-boundary helpers in result.ts. `Response` objects are
// constructed directly — no MSW needed, since these functions operate on a
// Response, not on a URL.

describe('parseApiError', () => {
  it('maps a recognized wire code to that ApiError code', async () => {
    const res = new Response(
      JSON.stringify({ error: 'Session is closed.', code: 'session_closed' }),
      { status: 409 },
    );
    expect(await parseApiError(res)).toEqual({
      code: 'session_closed',
      message: 'Session is closed.',
    });
  });

  it('falls back to `unknown` for a code not in the registry', async () => {
    const res = new Response(
      JSON.stringify({ error: 'Something odd', code: 'brand_new_code' }),
      { status: 400 },
    );
    expect(await parseApiError(res)).toEqual({
      code: 'unknown',
      message: 'Something odd',
    });
  });

  it('falls back to `unknown` when the envelope carries no code', async () => {
    const res = new Response(JSON.stringify({ error: 'No code here' }), {
      status: 400,
    });
    expect(await parseApiError(res)).toEqual({
      code: 'unknown',
      message: 'No code here',
    });
  });

  it('degrades to an HTTP-status message when the body is not JSON', async () => {
    const res = new Response('<<gateway error page>>', { status: 502 });
    expect(await parseApiError(res)).toEqual({
      code: 'unknown',
      message: 'Request failed (HTTP 502)',
    });
  });

  it('degrades when the JSON body is not the error envelope shape', async () => {
    const res = new Response(JSON.stringify({ unexpected: true }), {
      status: 500,
    });
    expect(await parseApiError(res)).toEqual({
      code: 'unknown',
      message: 'Request failed (HTTP 500)',
    });
  });
});

describe('parseBody', () => {
  const schema = z.object({ name: z.string(), count: z.number() });

  it('returns the parsed value for a body that matches the schema', async () => {
    const res = new Response(JSON.stringify({ name: 'ok', count: 3 }));
    expect(await parseBody(schema, res)).toEqual({ name: 'ok', count: 3 });
  });

  it('throws a response_shape_mismatch ApiErrorException on a schema failure', async () => {
    const res = new Response(JSON.stringify({ name: 'ok' })); // count missing
    try {
      await parseBody(schema, res);
      expect.unreachable('parseBody should have thrown');
    } catch (e) {
      expect(e).toBeInstanceOf(ApiErrorException);
      expect((e as ApiErrorException).apiError.code).toBe(
        'response_shape_mismatch',
      );
      // The ZodError is preserved as `cause` so the original detail survives.
      expect((e as ApiErrorException).cause).toBeInstanceOf(ZodError);
    }
  });

  it('throws response_shape_mismatch when the body is not JSON', async () => {
    const res = new Response('not json at all');
    try {
      await parseBody(schema, res);
      expect.unreachable('parseBody should have thrown');
    } catch (e) {
      expect(e).toBeInstanceOf(ApiErrorException);
      expect((e as ApiErrorException).apiError.code).toBe(
        'response_shape_mismatch',
      );
    }
  });
});

describe('ApiErrorException', () => {
  it('is an Error whose message is the ApiError message', () => {
    const ex = new ApiErrorException({
      code: 'not_found',
      message: 'Resource is gone',
    });
    expect(ex).toBeInstanceOf(Error);
    expect(ex.message).toBe('Resource is gone');
    expect(ex.apiError).toEqual({
      code: 'not_found',
      message: 'Resource is gone',
    });
  });
});
