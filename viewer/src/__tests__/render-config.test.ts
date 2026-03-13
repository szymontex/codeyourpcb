import { describe, it, expect } from 'vitest';
import {
  createDefaultRenderConfig,
  getLodTier,
  buildPadNetMap,
  LodTier,
} from '../render-config';
import type { NetInfo } from '../types';

describe('createDefaultRenderConfig', () => {
  it('returns a valid config with all required fields', () => {
    const config = createDefaultRenderConfig();

    expect(config.layerColors).toBeDefined();
    expect(config.layerColors.topCopper).toBe('#C83434');
    expect(config.layerColors.bottomCopper).toBe('#3434C8');
    expect(config.layerColors.silkscreen).toBe('#C8C800');
    expect(config.layerColors.via).toBe('#808080');
    expect(config.layerColors.drill).toBe('#FFFFFF');

    expect(config.fontConfig.refdesWorldSize).toBeGreaterThan(0);
    expect(config.fontConfig.padNumberMinScreenPx).toBeGreaterThan(0);
    expect(config.fontConfig.netLabelMinSegmentPx).toBeGreaterThan(0);

    expect(config.lodThresholds.medium).toBeGreaterThan(0);
    expect(config.lodThresholds.close).toBeGreaterThan(config.lodThresholds.medium);
    expect(config.lodThresholds.detail).toBeGreaterThan(config.lodThresholds.close);
  });
});

describe('getLodTier', () => {
  const config = createDefaultRenderConfig();

  it('returns Far at very low scale', () => {
    expect(getLodTier(0.00001, config)).toBe(LodTier.Far);
  });

  it('returns Medium at medium scale', () => {
    // Just above medium threshold
    expect(getLodTier(config.lodThresholds.medium + 0.000001, config)).toBe(LodTier.Medium);
  });

  it('returns Close at close scale', () => {
    expect(getLodTier(config.lodThresholds.close + 0.000001, config)).toBe(LodTier.Close);
  });

  it('returns Detail at high scale', () => {
    expect(getLodTier(config.lodThresholds.detail + 0.000001, config)).toBe(LodTier.Detail);
  });

  it('returns exact boundary — medium threshold yields Medium', () => {
    expect(getLodTier(config.lodThresholds.medium, config)).toBe(LodTier.Medium);
  });

  it('returns exact boundary — close threshold yields Close', () => {
    expect(getLodTier(config.lodThresholds.close, config)).toBe(LodTier.Close);
  });

  it('returns exact boundary — detail threshold yields Detail', () => {
    expect(getLodTier(config.lodThresholds.detail, config)).toBe(LodTier.Detail);
  });

  it('returns Far at zero scale', () => {
    expect(getLodTier(0, config)).toBe(LodTier.Far);
  });
});

describe('buildPadNetMap', () => {
  it('maps multi-pin component correctly', () => {
    const nets: NetInfo[] = [
      {
        name: 'VCC',
        id: 1,
        connections: [
          { component: 'U1', pin: '1' },
          { component: 'C1', pin: '1' },
        ],
      },
      {
        name: 'GND',
        id: 2,
        connections: [
          { component: 'U1', pin: '4' },
          { component: 'C1', pin: '2' },
        ],
      },
    ];

    const map = buildPadNetMap(nets);
    expect(map.get('U1.1')).toBe('VCC');
    expect(map.get('C1.1')).toBe('VCC');
    expect(map.get('U1.4')).toBe('GND');
    expect(map.get('C1.2')).toBe('GND');
    expect(map.size).toBe(4);
  });

  it('handles empty nets array', () => {
    const map = buildPadNetMap([]);
    expect(map.size).toBe(0);
  });

  it('handles net with no connections', () => {
    const nets: NetInfo[] = [
      { name: 'FLOATING', id: 10, connections: [] },
    ];
    const map = buildPadNetMap(nets);
    expect(map.size).toBe(0);
  });

  it('skips nets with empty name', () => {
    const nets: NetInfo[] = [
      {
        name: '',
        id: 0,
        connections: [{ component: 'U1', pin: '1' }],
      },
    ];
    const map = buildPadNetMap(nets);
    expect(map.size).toBe(0);
  });

  it('handles power nets with standard names', () => {
    const nets: NetInfo[] = [
      {
        name: '+3V3',
        id: 5,
        connections: [
          { component: 'U1', pin: '14' },
          { component: 'U2', pin: '7' },
          { component: 'C3', pin: '1' },
        ],
      },
    ];

    const map = buildPadNetMap(nets);
    expect(map.get('U1.14')).toBe('+3V3');
    expect(map.get('U2.7')).toBe('+3V3');
    expect(map.get('C3.1')).toBe('+3V3');
    expect(map.size).toBe(3);
  });

  it('last-write wins when same pin appears in multiple nets (data error)', () => {
    // This shouldn't happen in valid data, but the function should not throw
    const nets: NetInfo[] = [
      { name: 'NET_A', id: 1, connections: [{ component: 'U1', pin: '1' }] },
      { name: 'NET_B', id: 2, connections: [{ component: 'U1', pin: '1' }] },
    ];
    const map = buildPadNetMap(nets);
    // Last net wins
    expect(map.get('U1.1')).toBe('NET_B');
    expect(map.size).toBe(1);
  });
});
