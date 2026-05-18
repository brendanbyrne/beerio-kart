import { describe, it, expect } from 'vitest';
import { formatTime, parseTimeFields } from './time';

describe('formatTime', () => {
  it('formats a sub-minute time as M:SS.mmm', () => {
    expect(formatTime(12_345)).toBe('0:12.345');
  });

  it('formats a multi-minute time with no leading zero on minutes', () => {
    expect(formatTime(83_500)).toBe('1:23.500');
  });

  it('pads seconds and milliseconds to two and three digits', () => {
    expect(formatTime(5_007)).toBe('0:05.007');
  });

  it('returns an em dash for a negative time', () => {
    expect(formatTime(-1)).toBe('—');
  });
});

describe('parseTimeFields', () => {
  it('parses minute, second, and millisecond fields into total milliseconds', () => {
    expect(parseTimeFields('1', '23', '500')).toBe(83_500);
  });

  it('rejects non-numeric input', () => {
    expect(parseTimeFields('a', '23', '500')).toBeNull();
  });

  it('rejects seconds over 59', () => {
    expect(parseTimeFields('1', '60', '0')).toBeNull();
  });

  it('rejects milliseconds over 999', () => {
    expect(parseTimeFields('1', '0', '1000')).toBeNull();
  });

  it('rejects negative minutes', () => {
    expect(parseTimeFields('-1', '0', '0')).toBeNull();
  });

  it('round-trips a parsed time back through the formatter', () => {
    const ms = parseTimeFields('1', '32', '345');
    expect(ms).toBe(92_345);
    // `!` is fine in test code (typescript.md § 12) — the assertion above
    // already proves `ms` is non-null.
    expect(formatTime(ms!)).toBe('1:32.345');
  });
});
