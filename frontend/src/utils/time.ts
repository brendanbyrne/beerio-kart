/** Format milliseconds as M:SS.mmm (no leading zero on minutes). */
export function formatTime(ms: number): string {
  if (ms < 0) return '\u2014'
  const minutes = Math.floor(ms / 60000)
  const seconds = Math.floor((ms % 60000) / 1000)
  const millis = ms % 1000
  return `${minutes}:${seconds.toString().padStart(2, '0')}.${millis.toString().padStart(3, '0')}`
}

/** Parse M, SS, mmm fields into total milliseconds. Returns null if invalid. */
export function parseTimeFields(m: string, ss: string, mmm: string): number | null {
  const mins = parseInt(m, 10)
  const secs = parseInt(ss, 10)
  const ms = parseInt(mmm, 10)
  if (isNaN(mins) || isNaN(secs) || isNaN(ms)) return null
  if (secs > 59 || ms > 999 || mins < 0 || secs < 0 || ms < 0) return null
  return mins * 60000 + secs * 1000 + ms
}
