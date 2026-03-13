import { describe, it, expect } from 'vitest';
import { buildPadNetMap } from '../render-config';
import type { NetInfo } from '../types';

describe('buildPadNetMap — edge cases', () => {
  it('returns empty map for empty nets array', () => {
    const map = buildPadNetMap([]);
    expect(map.size).toBe(0);
    expect(map).toBeInstanceOf(Map);
  });

  it('skips net with no connections', () => {
    const nets: NetInfo[] = [
      { name: 'ORPHAN_NET', id: 1, connections: [] },
    ];
    const map = buildPadNetMap(nets);
    expect(map.size).toBe(0);
  });

  it('maps multi-pin IC (20 pins) correctly', () => {
    const connections = Array.from({ length: 20 }, (_, i) => ({
      component: 'U1',
      pin: String(i + 1),
    }));
    const nets: NetInfo[] = [
      { name: 'IC_BUS', id: 10, connections },
    ];

    const map = buildPadNetMap(nets);
    expect(map.size).toBe(20);
    for (let i = 1; i <= 20; i++) {
      expect(map.get(`U1.${i}`)).toBe('IC_BUS');
    }
  });

  it('handles through-hole component with alphanumeric pins (A1, B2)', () => {
    const nets: NetInfo[] = [
      {
        name: 'DATA_BUS',
        id: 5,
        connections: [
          { component: 'J1', pin: 'A1' },
          { component: 'J1', pin: 'A2' },
          { component: 'J1', pin: 'B1' },
          { component: 'J1', pin: 'B2' },
        ],
      },
    ];

    const map = buildPadNetMap(nets);
    expect(map.size).toBe(4);
    expect(map.get('J1.A1')).toBe('DATA_BUS');
    expect(map.get('J1.A2')).toBe('DATA_BUS');
    expect(map.get('J1.B1')).toBe('DATA_BUS');
    expect(map.get('J1.B2')).toBe('DATA_BUS');
  });

  it('last-write wins for duplicate pin refs across nets', () => {
    // Invalid data: same pin on two nets. Should not crash, last net wins.
    const nets: NetInfo[] = [
      { name: 'NET_FIRST', id: 1, connections: [{ component: 'U1', pin: '3' }] },
      { name: 'NET_SECOND', id: 2, connections: [{ component: 'U1', pin: '3' }] },
    ];

    const map = buildPadNetMap(nets);
    expect(map.get('U1.3')).toBe('NET_SECOND');
    expect(map.size).toBe(1);
  });

  it('maps power net connections (VCC, GND) correctly', () => {
    const nets: NetInfo[] = [
      {
        name: 'VCC',
        id: 100,
        connections: [
          { component: 'U1', pin: '14' },
          { component: 'C1', pin: '1' },
          { component: 'C2', pin: '1' },
          { component: 'R1', pin: '1' },
        ],
      },
      {
        name: 'GND',
        id: 101,
        connections: [
          { component: 'U1', pin: '7' },
          { component: 'C1', pin: '2' },
          { component: 'C2', pin: '2' },
          { component: 'R2', pin: '2' },
        ],
      },
    ];

    const map = buildPadNetMap(nets);
    expect(map.size).toBe(8);

    // VCC pins
    expect(map.get('U1.14')).toBe('VCC');
    expect(map.get('C1.1')).toBe('VCC');
    expect(map.get('C2.1')).toBe('VCC');
    expect(map.get('R1.1')).toBe('VCC');

    // GND pins
    expect(map.get('U1.7')).toBe('GND');
    expect(map.get('C1.2')).toBe('GND');
    expect(map.get('C2.2')).toBe('GND');
    expect(map.get('R2.2')).toBe('GND');
  });

  it('skips nets with empty string name', () => {
    const nets: NetInfo[] = [
      { name: '', id: 0, connections: [{ component: 'X1', pin: '1' }] },
      { name: 'VALID', id: 1, connections: [{ component: 'X2', pin: '1' }] },
    ];
    const map = buildPadNetMap(nets);
    expect(map.size).toBe(1);
    expect(map.has('X1.1')).toBe(false);
    expect(map.get('X2.1')).toBe('VALID');
  });

  it('handles multiple nets with shared components on different pins', () => {
    const nets: NetInfo[] = [
      {
        name: 'SDA',
        id: 20,
        connections: [
          { component: 'U1', pin: '5' },
          { component: 'R3', pin: '1' },
        ],
      },
      {
        name: 'SCL',
        id: 21,
        connections: [
          { component: 'U1', pin: '6' },
          { component: 'R4', pin: '1' },
        ],
      },
    ];

    const map = buildPadNetMap(nets);
    expect(map.size).toBe(4);
    expect(map.get('U1.5')).toBe('SDA');
    expect(map.get('U1.6')).toBe('SCL');
    expect(map.get('R3.1')).toBe('SDA');
    expect(map.get('R4.1')).toBe('SCL');
  });
});
