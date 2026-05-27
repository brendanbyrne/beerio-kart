import { describe, expect, it } from 'vitest';
import { readString } from './forms';

describe('readString', () => {
  it('returns the string value when the field holds text', () => {
    const fd = new FormData();
    fd.set('username', 'alice');
    expect(readString(fd, 'username')).toBe('alice');
  });

  it('returns "" when the field is missing', () => {
    const fd = new FormData();
    expect(readString(fd, 'missing')).toBe('');
  });

  it('returns "" when the field holds a File (defensive for non-text inputs)', () => {
    const fd = new FormData();
    fd.set('avatar', new File(['data'], 'a.png', { type: 'image/png' }));
    expect(readString(fd, 'avatar')).toBe('');
  });
});
