import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiErrorException } from './result';
import { createQueryClient } from './queryClient';

// The QueryCache `onError` is what keeps contract drift visible (typescript.md
// § 8) now that the read hooks degrade to empty/null instead of calling
// `logIfResponseShapeMismatch` themselves. Drive a query to error through the
// real client and assert the response-shape case logs while a network error
// stays quiet (the degrade-to-empty contract).

afterEach(() => {
  vi.restoreAllMocks();
});

describe('createQueryClient', () => {
  it('logs a response-shape mismatch surfaced by a failed query', async () => {
    const errorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const client = createQueryClient();
    const drift = new ApiErrorException({
      code: 'response_shape_mismatch',
      message: 'Response failed schema validation',
    });

    await expect(
      client.fetchQuery({
        queryKey: ['characters'],
        queryFn: () => Promise.reject(drift),
        retry: false,
      }),
    ).rejects.toBe(drift);

    // queryHash is the stringified key, so the log carries the resource name.
    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('characters'),
      drift,
    );
  });

  it('stays silent for a network error (degrade-to-empty, not drift)', async () => {
    const errorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const client = createQueryClient();
    const networkError = new TypeError('Failed to fetch');

    await expect(
      client.fetchQuery({
        queryKey: ['drink-types'],
        queryFn: () => Promise.reject(networkError),
        retry: false,
      }),
    ).rejects.toBe(networkError);

    expect(errorSpy).not.toHaveBeenCalled();
  });
});
