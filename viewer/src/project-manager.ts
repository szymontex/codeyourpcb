/**
 * Project Manager — startup screen with templates, recent files, and file open.
 *
 * Shows an overlay on app launch letting the user pick a template, open a recent
 * file, or import a .cypcb from disk. Dismissed automatically when a file loads.
 *
 * Debug surface: `window.__projectManager` exposes state for E2E tests.
 */

import { getPreference, setPreference, type RecentFileEntry } from './settings';
import { render, type RenderState } from './renderer';
import { createViewport, fitBoard } from './viewport';
import { createDefaultRenderConfig, buildPadNetMap } from './render-config';
import { createLayerVisibility } from './layers';
import type { BoardSnapshot } from './types';

// How many recent projects the app keeps and shows. One number, because a
// writer that caps at ten and a reader that renders everything disagree in
// front of the user.
const RECENT_FILES_SHOWN = 10;

// ---------------------------------------------------------------------------
// Template descriptors (static, bundled in public/templates/)
// ---------------------------------------------------------------------------

interface TemplateInfo {
  id: string;
  name: string;
  description: string;
  file: string;
}

const TEMPLATES: TemplateInfo[] = [
  {
    id: 'blink',
    name: 'Blink LED',
    description: '555 timer astable circuit — LED blinks at ~1.4 Hz',
    file: 'blink.cypcb',
  },
  {
    id: 'power-indicator',
    name: 'Power Indicator',
    description: 'Simple LED power indicator with current-limiting resistor',
    file: 'power-indicator.cypcb',
  },
  {
    id: 'simple-psu',
    name: 'Simple PSU',
    description: '5V regulated power supply — 7805 with filter caps',
    file: 'simple-psu.cypcb',
  },
  {
    id: 'alignment-test',
    name: '3D Alignment Test',
    description: 'Components at corners & edges — verify 3D model placement',
    file: 'alignment-test.cypcb',
  },
];

// ---------------------------------------------------------------------------
// Blank project scaffold
// ---------------------------------------------------------------------------

const BLANK_SCAFFOLD = `version 1

board untitled {
    size 50mm x 50mm
    layers 2
}
`;

// ---------------------------------------------------------------------------
// Callbacks from the host (main.ts)
// ---------------------------------------------------------------------------

export interface ProjectManagerCallbacks {
  onOpenFile: () => void;
  onLoadTemplate: (source: string, name: string) => void;
  onNewBlank: (source: string) => void;
  onLoadRecent: (source: string, name: string) => void;
  onRequestFileList: () => void;
  onOpenProjectFile: (path: string, name: string) => void;
  /** Called when PM opens — host should refresh thumbnail for the current file */
  onRefreshThumbnail?: () => void;
}

// ---------------------------------------------------------------------------
// Module state
// ---------------------------------------------------------------------------

let overlay: HTMLElement | null = null;
let recentListEl: HTMLElement | null = null;
let projectListEl: HTMLElement | null = null;
let projectSectionEl: HTMLElement | null = null;
let callbacks: ProjectManagerCallbacks | null = null;
let visible = false;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Wire DOM event handlers on the project manager overlay.
 * Call once after WASM init.
 */
export function initProjectManager(cb: ProjectManagerCallbacks): void {
  callbacks = cb;

  overlay = document.getElementById('project-manager');
  recentListEl = document.getElementById('pm-recent-list');
  projectListEl = document.getElementById('pm-project-list');
  projectSectionEl = document.getElementById('pm-projects-section');

  if (!overlay) {
    console.warn('[ProjectManager] #project-manager element not found');
    return;
  }

  // Template card clicks
  const templateCards = overlay.querySelectorAll<HTMLElement>('[data-template]');
  templateCards.forEach((card) => {
    card.addEventListener('click', () => {
      const templateId = card.dataset.template!;
      handleTemplateClick(templateId);
    });
  });

  // Blank card
  const blankCard = overlay.querySelector<HTMLElement>('[data-template-blank]');
  if (blankCard) {
    blankCard.addEventListener('click', () => {
      callbacks?.onNewBlank(BLANK_SCAFFOLD);
    });
  }

  // Open file button
  const openBtn = overlay.querySelector<HTMLElement>('#pm-open-btn');
  if (openBtn) {
    openBtn.addEventListener('click', () => {
      callbacks?.onOpenFile();
    });
  }

  exposeDebugSurface();
}

/**
 * Show the project manager overlay and populate dynamic content.
 */
export function showProjectManager(): void {
  if (!overlay) return;

  // Refresh thumbnail for current file before displaying
  callbacks?.onRefreshThumbnail?.();

  populateRecentFiles();
  overlay.classList.remove('hidden');
  visible = true;

  // Request workspace file list from dev server
  callbacks?.onRequestFileList();

  exposeDebugSurface();
}

/**
 * Hide the project manager overlay.
 */
export function hideProjectManager(): void {
  if (!overlay) return;

  overlay.classList.add('hidden');
  visible = false;
  exposeDebugSurface();
}

/**
 * Record a file open in the recent files list.
 * Generates a thumbnail from the current board state if a snapshot and canvas
 * context are available.
 *
 * @param name     Display name (filename)
 * @param snapshot Current board snapshot (optional, for thumbnail)
 * @param renderState Partial render state for thumbnail generation (optional)
 */
export function addRecentFile(
  name: string,
  snapshot?: BoardSnapshot | null,
  renderState?: Partial<RenderState> | null,
  source?: string | null,
): void {
  const recentFiles = getPreference('recentFiles');

  // Remove existing entry with same name
  const filtered = recentFiles.filter((e) => e.name !== name);

  let thumbnail: string | null = null;
  if (snapshot && renderState) {
    try {
      thumbnail = generateThumbnail(snapshot, renderState);
    } catch (e) {
      console.warn('[ProjectManager] Thumbnail generation failed, using placeholder', e);
    }
  }

  const entry: RecentFileEntry = {
    name,
    timestamp: Date.now(),
    thumbnail,
    source: source ?? null,
  };

  filtered.unshift(entry);

  const capped = filtered.slice(0, RECENT_FILES_SHOWN);

  setPreference('recentFiles', capped);
  exposeDebugSurface();
}

/**
 * Generate a 200×150 thumbnail of the board via offscreen canvas.
 */
export function generateThumbnail(
  snapshot: BoardSnapshot,
  partialState: Partial<RenderState>,
): string {
  const width = 400;
  const height = 300;

  const offscreen = document.createElement('canvas');
  offscreen.width = width;
  offscreen.height = height;
  const ctx = offscreen.getContext('2d')!;

  // Build a minimal viewport fitted to the board
  let vp = createViewport(width, height);
  if (snapshot.board) {
    vp = fitBoard(vp, snapshot.board.width_nm, snapshot.board.height_nm, 10);
  }

  const layers = createLayerVisibility();
  const renderConfig = createDefaultRenderConfig();
  const padNetMap = snapshot.nets ? buildPadNetMap(snapshot.nets) : new Map<string, string>();

  const base: RenderState = {
    snapshot,
    viewport: vp,
    layers,
    selectedRefdes: null,
    showViolations: false,
    showRatsnest: false,
    colorByNet: true,
    selectedTraceId: null,
    hoveredTraceId: null,
    labelPosition: null,
    routing: null,
    highlightedNet: null,
    activeResizeHandle: null,
    renderConfig,
    padNetMap,
    gridVisible: false,
    gridVisualSpacing: 1_000_000,
    showNetLabels: false,
  };

  // Apply partial overrides but force our thumbnail viewport
  const state: RenderState = {
    ...base,
    ...partialState,
    snapshot,
    viewport: vp,
  };

  render(ctx, state);

  return offscreen.toDataURL('image/png');
}

/**
 * Update the workspace project file list (called when server responds).
 */
export function updateProjectFiles(files: Array<{ path: string; name: string }>): void {
  if (!projectListEl || !projectSectionEl) return;

  projectListEl.textContent = '';

  if (files.length === 0) {
    // Hide the section entirely when no workspace files
    projectSectionEl.style.display = 'none';
    return;
  }

  projectSectionEl.style.display = '';

  files.forEach((file) => {
    const item = document.createElement('div');
    item.className = 'pm-project-item';
    item.addEventListener('click', () => {
      callbacks?.onOpenProjectFile(file.path, file.name);
    });

    const icon = document.createElement('div');
    icon.className = 'pm-project-icon';
    icon.textContent = '⬡';
    item.appendChild(icon);

    const nameEl = document.createElement('div');
    nameEl.className = 'pm-project-name';
    nameEl.textContent = file.name.replace(/\.cypcb$/, '');
    item.appendChild(nameEl);

    projectListEl!.appendChild(item);
  });
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async function handleTemplateClick(templateId: string): Promise<void> {
  const template = TEMPLATES.find((t) => t.id === templateId);
  if (!template || !callbacks) return;

  try {
    const resp = await fetch(`/templates/${template.file}`);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const source = await resp.text();
    callbacks.onLoadTemplate(source, template.name);
  } catch (e) {
    console.error(`[ProjectManager] Failed to load template ${template.file}:`, e);
  }
}

function populateRecentFiles(): void {
  if (!recentListEl) return;

  const recentFiles = getPreference('recentFiles');
  const sectionEl = document.getElementById('pm-recent-section');

  // Clear existing content
  recentListEl.textContent = '';

  if (recentFiles.length === 0) {
    // Hide the entire section when empty
    if (sectionEl) sectionEl.style.display = 'none';
    return;
  }

  if (sectionEl) sectionEl.style.display = '';

  // The same limit the writer applies. Stored state can hold more - an older
  // build wrote a longer list, or a test seeded one - and rendering all of it
  // shows a user rows the app will silently drop on their next save.
  recentFiles.slice(0, RECENT_FILES_SHOWN).forEach((entry) => {
    const item = document.createElement('div');
    item.className = 'pm-recent-item';
    item.addEventListener('click', () => {
      if (entry.source && callbacks) {
        callbacks.onLoadRecent(entry.source, entry.name);
      } else {
        console.log(`[ProjectManager] Recent file has no stored source: ${entry.name}`);
      }
    });

    if (entry.thumbnail) {
      const thumb = document.createElement('img');
      thumb.className = 'pm-recent-thumb';
      thumb.src = entry.thumbnail;
      thumb.alt = entry.name;
      item.appendChild(thumb);
    } else {
      const placeholder = document.createElement('div');
      placeholder.className = 'pm-recent-thumb-placeholder';
      placeholder.textContent = '📄';
      item.appendChild(placeholder);
    }

    const info = document.createElement('div');
    info.className = 'pm-recent-info';

    const nameEl = document.createElement('div');
    nameEl.className = 'pm-recent-name';
    nameEl.textContent = entry.name.replace(/\.cypcb$/, '');
    info.appendChild(nameEl);

    const dateEl = document.createElement('div');
    dateEl.className = 'pm-recent-date';
    dateEl.textContent = formatRelativeDate(entry.timestamp);
    info.appendChild(dateEl);

    item.appendChild(info);
    recentListEl!.appendChild(item);
  });
}

function formatRelativeDate(ts: number): string {
  const diff = Date.now() - ts;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(ts).toLocaleDateString();
}

// ---------------------------------------------------------------------------
// Debug surface
// ---------------------------------------------------------------------------

function exposeDebugSurface(): void {
  if (typeof window === 'undefined') return;

  (window as any).__projectManager = {
    get visible() { return visible; },
    get recentFiles() { return getPreference('recentFiles'); },
    get templateCount() { return TEMPLATES.length; },
    show: showProjectManager,
    hide: hideProjectManager,
  };
}
