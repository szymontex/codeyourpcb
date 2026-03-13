import { describe, it, expect } from 'vitest';
import {
  formatDimension,
  parseUserDimension,
  NM_PER_MM,
  NM_PER_MIL,
  NM_PER_UM,
} from '../units';

describe('formatDimension', () => {
  it('formats nanometers to mm', () => {
    expect(formatDimension(2_540_000, 'mm')).toBe('2.54mm');
    expect(formatDimension(1_000_000, 'mm')).toBe('1mm');
    expect(formatDimension(500_000, 'mm')).toBe('0.5mm');
  });

  it('formats nanometers to mil', () => {
    expect(formatDimension(2_540_000, 'mil')).toBe('100mil');
    expect(formatDimension(25_400, 'mil')).toBe('1mil');
    expect(formatDimension(254_000, 'mil')).toBe('10mil');
    // Non-exact value shows needed precision
    expect(formatDimension(1_000_000, 'mil')).toBe('39.3701mil');
  });

  it('formats nanometers to µm', () => {
    expect(formatDimension(2_540_000, 'µm')).toBe('2540µm');
    expect(formatDimension(1_000, 'µm')).toBe('1µm');
    expect(formatDimension(500, 'µm')).toBe('0.5µm');
  });

  it('handles zero', () => {
    expect(formatDimension(0, 'mm')).toBe('0mm');
    expect(formatDimension(0, 'mil')).toBe('0mil');
    expect(formatDimension(0, 'µm')).toBe('0µm');
  });

  it('handles negative values', () => {
    expect(formatDimension(-1_000_000, 'mm')).toBe('-1mm');
    expect(formatDimension(-25_400, 'mil')).toBe('-1mil');
    expect(formatDimension(-1_000, 'µm')).toBe('-1µm');
  });

  it('handles very large values', () => {
    expect(formatDimension(100_000_000_000, 'mm')).toBe('100000mm');
    expect(formatDimension(100_000_000_000, 'mil')).toContain('mil');
  });
});

describe('parseUserDimension', () => {
  it('parses mm values to nanometers', () => {
    expect(parseUserDimension('2.54mm')).toBe(2_540_000);
    expect(parseUserDimension('1mm')).toBe(1_000_000);
    expect(parseUserDimension('0.5mm')).toBe(500_000);
  });

  it('parses mil values to nanometers', () => {
    expect(parseUserDimension('100mil')).toBe(2_540_000);
    expect(parseUserDimension('1mil')).toBe(25_400);
    expect(parseUserDimension('50mil')).toBe(1_270_000);
  });

  it('parses µm values to nanometers', () => {
    expect(parseUserDimension('2540µm')).toBe(2_540_000);
    expect(parseUserDimension('1µm')).toBe(1_000);
  });

  it('parses um (ASCII alias) values to nanometers', () => {
    expect(parseUserDimension('2540um')).toBe(2_540_000);
    expect(parseUserDimension('1um')).toBe(1_000);
  });

  it('round-trips with formatDimension for mm', () => {
    const values = [1_000_000, 2_540_000, 25_400, 500_000];
    for (const nm of values) {
      expect(parseUserDimension(formatDimension(nm, 'mm'))).toBe(nm);
    }
  });

  it('round-trips with formatDimension for µm', () => {
    const values = [1_000_000, 2_540_000, 25_400, 500_000];
    for (const nm of values) {
      expect(parseUserDimension(formatDimension(nm, 'µm'))).toBe(nm);
    }
  });

  it('round-trips with formatDimension for mil (exact multiples)', () => {
    // Use values that are exact multiples of NM_PER_MIL (25_400)
    const values = [25_400, 254_000, 2_540_000, 1_270_000];
    for (const nm of values) {
      expect(parseUserDimension(formatDimension(nm, 'mil'))).toBe(nm);
    }
  });

  it('returns null for invalid input', () => {
    expect(parseUserDimension('')).toBeNull();
    expect(parseUserDimension('abc')).toBeNull();
    expect(parseUserDimension('mm')).toBeNull();
    expect(parseUserDimension('12')).toBeNull();
    expect(parseUserDimension('12px')).toBeNull();
  });

  it('tolerates whitespace', () => {
    expect(parseUserDimension('  2.54mm  ')).toBe(2_540_000);
    expect(parseUserDimension(' 100 mil')).toBe(2_540_000);
    expect(parseUserDimension('  1  mm  ')).toBe(1_000_000);
  });

  it('is case insensitive for unit suffix', () => {
    expect(parseUserDimension('2.54MM')).toBe(2_540_000);
    expect(parseUserDimension('100MIL')).toBe(2_540_000);
    expect(parseUserDimension('2540UM')).toBe(2_540_000);
  });

  it('handles negative values', () => {
    expect(parseUserDimension('-1mm')).toBe(-1_000_000);
    expect(parseUserDimension('-50mil')).toBe(-1_270_000);
  });
});

describe('conversion constants', () => {
  it('has correct nm/mm ratio', () => {
    expect(NM_PER_MM).toBe(1_000_000);
  });

  it('has correct nm/mil ratio', () => {
    expect(NM_PER_MIL).toBe(25_400);
  });

  it('has correct nm/µm ratio', () => {
    expect(NM_PER_UM).toBe(1_000);
  });
});
