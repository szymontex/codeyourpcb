import { describe, it, expect } from 'vitest';
import { mergeTracesIntoDsl, hasRoutedTracesSection } from '../trace-persist';

const SECTION_START = '// --- Routed traces (auto-generated) ---';
const SECTION_END = '// --- End routed traces ---';

describe('mergeTracesIntoDsl', () => {
  const baseSource = `version 1
board t { size 50mm x 30mm
layers 2 }
net VCC { R1.1 }
net GND { R1.2 }`;

  const traceBlock = `trace VCC {
    layer Top
    width 0.250000mm
    path 5.000000mm,10.000000mm -> 15.000000mm,10.000000mm
}`;

  it('returns source unchanged when no traces and no section', () => {
    const result = mergeTracesIntoDsl(baseSource, '');
    expect(result).toBe(baseSource);
  });

  it('appends trace section after source when no existing section', () => {
    const result = mergeTracesIntoDsl(baseSource, traceBlock);
    expect(result).toContain(SECTION_START);
    expect(result).toContain(SECTION_END);
    expect(result).toContain('trace VCC');
    expect(result).toContain('path 5.000000mm');
    // Source should be before trace section
    expect(result.indexOf('net GND')).toBeLessThan(result.indexOf(SECTION_START));
  });

  it('replaces existing trace section', () => {
    const sourceWithSection = `${baseSource}

${SECTION_START}
trace OLD {
    layer Top
    width 0.200000mm
    path 1mm,1mm -> 2mm,2mm
}
${SECTION_END}`;

    const result = mergeTracesIntoDsl(sourceWithSection, traceBlock);
    expect(result).toContain('trace VCC');
    expect(result).not.toContain('trace OLD');
    expect(result).toContain(SECTION_START);
    expect(result).toContain(SECTION_END);
  });

  it('removes section when traces are empty', () => {
    const sourceWithSection = `${baseSource}

${SECTION_START}
trace VCC {
    layer Top
    width 0.250000mm
    path 5mm,10mm -> 15mm,10mm
}
${SECTION_END}
`;

    const result = mergeTracesIntoDsl(sourceWithSection, '');
    expect(result).not.toContain(SECTION_START);
    expect(result).not.toContain(SECTION_END);
    expect(result).not.toContain('trace VCC');
    expect(result).toContain('net GND'); // Original content preserved
  });

  it('preserves hand-written trace blocks outside section', () => {
    const sourceWithHandTrace = `${baseSource}

trace MANUAL {
    from R1.1
    to R1.2
    layer Top
    width 0.3mm
    locked
}`;

    const result = mergeTracesIntoDsl(sourceWithHandTrace, traceBlock);
    expect(result).toContain('trace MANUAL'); // Hand-written preserved
    expect(result).toContain('trace VCC'); // Auto-generated added
    expect(result).toContain(SECTION_START);
  });
});

describe('hasRoutedTracesSection', () => {
  it('returns false for source without section', () => {
    expect(hasRoutedTracesSection('version 1\nboard t { }')).toBe(false);
  });

  it('returns true for source with section', () => {
    const source = `version 1\n${SECTION_START}\ntrace X { }\n${SECTION_END}`;
    expect(hasRoutedTracesSection(source)).toBe(true);
  });
});
