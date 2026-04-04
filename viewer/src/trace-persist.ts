/**
 * Trace persistence — merge exported DSL trace blocks into .cypcb source.
 *
 * Uses section markers to identify the auto-generated trace region:
 *   // --- Routed traces (auto-generated) ---
 *   ... trace blocks ...
 *   // --- End routed traces ---
 *
 * Traces outside the marked section (hand-written) are preserved.
 */

const SECTION_START = '// --- Routed traces (auto-generated) ---';
const SECTION_END = '// --- End routed traces ---';

/**
 * Merge exported trace DSL into a .cypcb source string.
 *
 * - If `exportedTraces` is empty and no section exists, returns source unchanged.
 * - If `exportedTraces` is empty and a section exists, removes the section.
 * - If a section already exists, replaces its content.
 * - If no section exists, appends after the last top-level block.
 *
 * @param originalSource  The current .cypcb file content
 * @param exportedTraces  DSL trace blocks from engine.export_traces_as_dsl()
 * @returns               Merged source string
 */
export function mergeTracesIntoDsl(
  originalSource: string,
  exportedTraces: string,
): string {
  const startIdx = originalSource.indexOf(SECTION_START);
  const endIdx = originalSource.indexOf(SECTION_END);

  const hasSection = startIdx !== -1 && endIdx !== -1 && endIdx > startIdx;
  const tracesEmpty = !exportedTraces || exportedTraces.trim().length === 0;

  if (tracesEmpty && !hasSection) {
    // Nothing to add, nothing to remove
    return originalSource;
  }

  if (tracesEmpty && hasSection) {
    // Remove existing section (including markers and surrounding blank lines)
    const before = originalSource.substring(0, startIdx).replace(/\n+$/, '\n');
    const after = originalSource.substring(endIdx + SECTION_END.length).replace(/^\n+/, '');
    return before + after;
  }

  // Build the new section
  const newSection = `${SECTION_START}\n${exportedTraces.trim()}\n${SECTION_END}\n`;

  if (hasSection) {
    // Replace existing section
    const before = originalSource.substring(0, startIdx);
    const after = originalSource.substring(endIdx + SECTION_END.length);
    return before + newSection + after;
  }

  // No existing section — append after the last content
  // Ensure there's a blank line separator
  const trimmed = originalSource.replace(/\s+$/, '');
  return trimmed + '\n\n' + newSection;
}

/**
 * Check if a source string contains the routed traces section.
 */
export function hasRoutedTracesSection(source: string): boolean {
  return source.includes(SECTION_START) && source.includes(SECTION_END);
}

/**
 * Update the routed traces section in a Monaco editor instance without
 * resetting cursor position.
 *
 * Uses `executeEdits` for a targeted replacement of just the section,
 * wrapped in `suppressSync` to prevent a parse→render loop.
 *
 * If the editor has no section yet and traces are non-empty, appends one.
 * If traces are empty and a section exists, removes it.
 *
 * @returns true if the editor was modified, false if no change needed
 */
export function syncTracesToEditor(
  editor: any,           // Monaco IStandaloneCodeEditor
  exportedTraces: string,
  suppressSync: { value: boolean },
): boolean {
  const model = editor.getModel();
  if (!model) return false;

  const fullText = model.getValue();
  const tracesEmpty = !exportedTraces || exportedTraces.trim().length === 0;

  // Find the section markers in the editor text
  const startIdx = fullText.indexOf(SECTION_START);
  const endIdx = fullText.indexOf(SECTION_END);
  const hasSection = startIdx !== -1 && endIdx !== -1 && endIdx > startIdx;

  if (tracesEmpty && !hasSection) {
    return false; // Nothing to do
  }

  // Build the new section text (or empty for removal)
  let newSectionText: string;
  if (tracesEmpty) {
    newSectionText = '';
  } else {
    newSectionText = `${SECTION_START}\n${exportedTraces.trim()}\n${SECTION_END}\n`;
  }

  // Convert character offsets to Monaco line/column positions
  let startLine: number;
  let startCol: number;
  let endLine: number;
  let endCol: number;

  if (hasSection) {
    // Find leading blank line (if any) before section start
    let sectionStart = startIdx;
    // Include one preceding newline so we don't accumulate blank lines
    if (sectionStart > 0 && fullText[sectionStart - 1] === '\n') {
      sectionStart--;
      if (sectionStart > 0 && fullText[sectionStart - 1] === '\n') {
        sectionStart--; // eat double newline before section
      }
    }
    const sectionEnd = endIdx + SECTION_END.length;
    // Also eat trailing newline
    const actualEnd = sectionEnd < fullText.length && fullText[sectionEnd] === '\n'
      ? sectionEnd + 1
      : sectionEnd;

    const startPos = model.getPositionAt(sectionStart);
    const endPos = model.getPositionAt(actualEnd);
    startLine = startPos.lineNumber;
    startCol = startPos.column;
    endLine = endPos.lineNumber;
    endCol = endPos.column;

    // Prepend separator if replacing
    if (!tracesEmpty) {
      newSectionText = '\n\n' + newSectionText;
    }
  } else {
    // No existing section — append at end of file
    const lineCount = model.getLineCount();
    const lastLineLen = model.getLineMaxColumn(lineCount);
    startLine = lineCount;
    startCol = lastLineLen;
    endLine = lineCount;
    endCol = lastLineLen;
    newSectionText = '\n\n' + newSectionText;
  }

  // Execute the targeted edit
  suppressSync.value = true;
  editor.executeEdits('trace-persist', [{
    range: {
      startLineNumber: startLine,
      startColumn: startCol,
      endLineNumber: endLine,
      endColumn: endCol,
    },
    text: newSectionText,
  }]);
  suppressSync.value = false;

  return true;
}
