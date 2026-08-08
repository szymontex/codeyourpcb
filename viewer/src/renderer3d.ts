/**
 * 3D PCB Board Renderer
 *
 * Renders the PCB board as a 3D scene with Three.js.
 * Coordinate system: X/Y from board data (mm), Z is stack-up axis (Z-up).
 * Board bottom face at Z=0, top face at Z=1.6mm.
 *
 * Copper layers:
 *   Bottom copper: Z = 0.035 mm (copper thickness above board bottom)
 *   Top copper:    Z = 1.565 mm (board thickness minus copper thickness)
 *   Pads get slight Z-offset above traces to prevent Z-fighting.
 */

import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import type { BoardSnapshot, TraceInfo, ViaInfo, ComponentInfo } from './types';
import {
  LAYER_COLORS,
  LAYER_MASK,
  INNER_LAYER_COLORS,
  innerLayerDepth,
  innerLayerIndex,
  viaSpanDepths,
  type LayerVisibility,
} from './layers';
import { fetch3DModelByUuid } from './jlcpcb';
import { parseEasyEdaOBJ } from './easyeda-obj-parser';

/** Component body height for SMD parts in mm */
const SMD_HEIGHT_MM = 1.2;

/** Component body height for THT parts in mm */
const THT_HEIGHT_MM = 5.0;

/** Standard PCB thickness in mm */
const BOARD_THICKNESS_MM = 1.6;

/** Copper layer thickness in mm */
const COPPER_THICKNESS_MM = 0.035;

/** Solder mask thickness in mm (KiCad DEFAULT_TECH_LAYER_THICKNESS) */
const MASK_THICKNESS_MM = 0.025;

/**
 * Z-layer stack (KiCad convention: board centered at Z=0)
 *
 *   F_Silk     +0.86mm   ─── silkscreen on top
 *   F_Mask     +0.835mm  ─── solder mask (green, semi-transparent)
 *   F_Cu       +0.8mm    ─── front copper (traces, pads)
 *   ─── Board top ───  +0.8mm
 *        FR-4 substrate  1.6mm
 *   ─── Board bot ───  -0.8mm
 *   B_Cu       -0.8mm    ─── back copper
 *   B_Mask     -0.835mm  ─── solder mask bottom
 */
const BOARD_TOP_Z = BOARD_THICKNESS_MM / 2;   // +0.8
const BOARD_BOT_Z = -BOARD_THICKNESS_MM / 2;  // -0.8

// Front copper: sits on board top surface
const F_COPPER_BOT_Z = BOARD_TOP_Z;
const F_COPPER_TOP_Z = BOARD_TOP_Z + COPPER_THICKNESS_MM;

// Back copper: sits on board bottom surface
const B_COPPER_TOP_Z = BOARD_BOT_Z;
const B_COPPER_BOT_Z = BOARD_BOT_Z - COPPER_THICKNESS_MM;

// Solder mask: sits on top of copper
const F_MASK_Z = F_COPPER_TOP_Z + 0.001;  // tiny offset to avoid z-fight
const B_MASK_Z = B_COPPER_BOT_Z - 0.001;

// Pads: slightly above mask to be visible through openings
const Z_TOP_PAD = F_COPPER_TOP_Z + 0.003;
const Z_BOTTOM_PAD = B_COPPER_BOT_Z - 0.003;


/** Nanometers to millimeters conversion factor */
const NM_TO_MM = 1e-6;

/** PCB solder mask green (bright KiCad green) */
const PCB_GREEN = 0x1a7a3a;


/** Parse a CSS hex color string to a Three.js Color */
/**
 * Create a merged copper mesh from a flat positions array and add it to a group.
 * Used for both traces and pads on each layer to avoid duplicating the
 * BufferGeometry + MeshStandardMaterial construction.
 */
function addCopperMesh(
  positions: number[],
  _colorHex: string,
  name: string,
  group: THREE.Group,
): void {
  if (positions.length === 0) return;
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geo.computeVertexNormals();
  // KiCad copper: high metalness, warm bronze, polished
  const mat = new THREE.MeshPhysicalMaterial({
    color: 0xB87333,        // Warm copper bronze (184, 115, 51)
    metalness: 0.95,        // Very metallic (KiCad: specular = color*0.75+0.25)
    roughness: 0.15,        // Polished (KiCad: shininess = 51.2/128)
    reflectivity: 0.9,
    side: THREE.DoubleSide,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.name = name;
  group.add(mesh);
}

/**
 * Every drilled hole a board's pads carry, in millimetres, with the component
 * rotation already applied.
 *
 * The 3D view built drilled cylinders for vias and nothing else, so a board
 * full of through-hole parts came out solid: a connector's pins showed their
 * copper on both faces with no hole between them, and a mounting hole - which
 * has no copper at all - showed nothing whatsoever.
 *
 * Kept separate from the three.js work so the geometry decision can be
 * checked without a WebGL context: what is drilled, where, and how wide.
 */
export function padDrills(
  components: ComponentInfo[],
): { x: number; y: number; diameter: number }[] {
  const drills: { x: number; y: number; diameter: number }[] = [];

  for (const comp of components) {
    const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
    const cosR = Math.cos(radians);
    const sinR = Math.sin(radians);

    for (const pad of comp.pads) {
      if (!pad.drill_nm) continue;

      const padXmm = pad.x_nm * NM_TO_MM;
      const padYmm = pad.y_nm * NM_TO_MM;
      drills.push({
        x: comp.x_nm * NM_TO_MM + (padXmm * cosR - padYmm * sinR),
        y: comp.y_nm * NM_TO_MM + (padXmm * sinR + padYmm * cosR),
        diameter: pad.drill_nm * NM_TO_MM,
      });
    }
  }

  return drills;
}

export class Renderer3D {
  private renderer: THREE.WebGLRenderer | null = null;
  private scene: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private container: HTMLElement | null = null;
  private animFrameId: number | null = null;
  private active = false;
  private boardGroup: THREE.Group | null = null;

  /** Named layer groups for visibility toggling */
  private layerGroups: Map<string, THREE.Group> = new Map();

  /** FPS tracking */
  private lastFrameTime = 0;
  private frameCount = 0;
  private currentFps = 0;
  private fpsLogTimer = 0;

  /** Geometry counts for debug surface */
  private _componentCount = 0;
  private _traceSegmentCount = 0;
  private _padCount = 0;
  private _viaCount = 0;
  private _padDrillCount = 0;

  /** Loaded GLTF models: refdes → Group */
  private loadedModels: Map<string, THREE.Group> = new Map();

  /** Count of OBJ models loaded (subset of loadedModels) */
  private _objModelCount = 0;

  /**
   * Initialize the 3D renderer inside the given container element.
   * Creates WebGL renderer, scene, camera, controls, and lighting.
   */
  init(container: HTMLElement): void {
    this.container = container;

    // Create WebGL renderer
    try {
      this.renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: false,
      });
    } catch (e) {
      console.error('[3D] WebGL not available', e);
      throw new Error('WebGL not available');
    }

    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setSize(container.clientWidth, container.clientHeight);
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.0;
    this.renderer.domElement.style.position = 'absolute';
    this.renderer.domElement.style.top = '0';
    this.renderer.domElement.style.left = '0';
    this.renderer.domElement.id = 'three-canvas';
    container.appendChild(this.renderer.domElement);

    // Scene
    this.scene = new THREE.Scene();
    this.setBackgroundFromCSS();

    // Camera — perspective, looking down at board
    const aspect = container.clientWidth / container.clientHeight;
    this.camera = new THREE.PerspectiveCamera(45, aspect, 0.1, 10000);
    this.camera.position.set(0, 0, 100);
    this.camera.up.set(0, 1, 0);

    // OrbitControls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;
    this.controls.zoomSpeed = 1.8;
    this.controls.target.set(0, 0, 0);
    this.controls.update();

    // Lighting — ambient + directional for even illumination
    // KiCad-style 3-point lighting: bright ambient + two directional
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.8);
    this.scene.add(ambientLight);

    const dirLight1 = new THREE.DirectionalLight(0xffffff, 1.0);
    dirLight1.position.set(50, 80, 100);
    this.scene.add(dirLight1);

    const dirLight2 = new THREE.DirectionalLight(0xffffff, 0.5);
    dirLight2.position.set(-30, -50, 60);
    this.scene.add(dirLight2);

    // Board group for easy clearing
    this.boardGroup = new THREE.Group();
    this.scene.add(this.boardGroup);

    // Resize handler
    this.onResize = this.onResize.bind(this);
    window.addEventListener('resize', this.onResize);

    // Start animation loop
    this.active = true;
    this.animate();

    // Expose debug surface
    this.updateDebugSurface();

    console.log('[3D] Initialized');
  }

  /**
   * Update board geometry from a BoardSnapshot.
   * Builds: substrate slab, copper traces, pads, vias, with layer groups for visibility.
   */
  updateBoard(snapshot: BoardSnapshot, layers: LayerVisibility): void {
    if (!this.boardGroup || !this.scene) return;

    // Clear existing board geometry and layer groups
    this.clearBoardGroup();
    this.layerGroups.clear();

    if (!snapshot.board) {
      console.log('[3D] No board data');
      this.updateDebugSurface();
      return;
    }

    const widthMm = snapshot.board.width_nm * NM_TO_MM;
    const heightMm = snapshot.board.height_nm * NM_TO_MM;

    // Board substrate — centered at Z=0 (KiCad convention)
    const subGeo = new THREE.BoxGeometry(widthMm, heightMm, BOARD_THICKNESS_MM);
    subGeo.translate(widthMm / 2, heightMm / 2, 0); // Z=0 center

    // FR-4 substrate — yellowish-brown visible at board edges
    const subMat = new THREE.MeshPhysicalMaterial({
      color: 0x8B7D3C,      // Light olive-tan FR-4
      roughness: 0.7,
      metalness: 0.0,
      clearcoat: 0.1,
      clearcoatRoughness: 0.5,
    });

    const boardMesh = new THREE.Mesh(subGeo, subMat);
    boardMesh.name = 'board-substrate';
    this.boardGroup.add(boardMesh);

    // Create layer groups
    const topGroup = new THREE.Group();
    topGroup.name = 'layer-top';
    const bottomGroup = new THREE.Group();
    bottomGroup.name = 'layer-bottom';
    const viaGroup = new THREE.Group();
    viaGroup.name = 'layer-vias';

    // A four-layer board's middle. `buildTraces` used to ask "is this Top?"
    // and put everything else on the bottom, so an inner trace was drawn as
    // bottom copper - not missing, which would be honest, but wrong.
    const innerCount = Math.max(0, (snapshot.board.layer_count ?? 2) - 2);
    const innerGroups: THREE.Group[] = [];
    for (let index = 0; index < innerCount; index++) {
      const group = new THREE.Group();
      group.name = `layer-inner-${index + 1}`;
      this.boardGroup.add(group);
      this.layerGroups.set(`inner${index + 1}`, group);
      innerGroups.push(group);
    }

    this.boardGroup.add(topGroup);
    this.boardGroup.add(bottomGroup);
    this.boardGroup.add(viaGroup);

    this.layerGroups.set('topCopper', topGroup);
    this.layerGroups.set('bottomCopper', bottomGroup);
    this.layerGroups.set('vias', viaGroup);

    // Build copper geometry (hidden under solder mask except at pads)
    this.buildTraces(snapshot.traces || [], topGroup, bottomGroup, innerGroups);
    this.buildPads(snapshot.components || [], topGroup, bottomGroup);
    this.buildVias(snapshot.vias || [], viaGroup, innerCount);

    // Build solder mask — green layer covering copper with openings at pads
    this.buildSolderMask(widthMm, heightMm, snapshot.components || [], topGroup, bottomGroup);

    // Build component bodies (on top layer group for now — all top-side)
    this.buildComponents(snapshot.components || [], topGroup);

    // Build silkscreen outlines from EasyEDA footprint data
    this.buildSilkscreen(snapshot.components || [], topGroup, bottomGroup);

    // Auto-load 3D models for components that have model_3d UUID
    this.autoLoad3DModels(snapshot.components || []);

    // Apply initial layer visibility
    this.updateLayerVisibility(layers);

    // Set orbit target to board center
    if (this.controls && this.camera) {
      const cx = widthMm / 2;
      const cy = heightMm / 2;
      const cz = 0; // board centered at Z=0
      this.controls.target.set(cx, cy, cz);

      // KiCad-like 45° isometric view
      const maxDim = Math.max(widthMm, heightMm);
      const distance = maxDim * 1.5;
      this.camera.position.set(cx + distance * 0.2, cy - distance * 0.4, distance * 0.7);
      this.controls.update();
    }

    const componentCount = snapshot.components?.length ?? 0;
    console.log(`[3D] Board updated: ${widthMm.toFixed(1)}x${heightMm.toFixed(1)} mm, ${componentCount} components`);
    this.updateDebugSurface();
  }

  /**
   * Update visibility of layer groups based on checkbox state.
   * Called from main.ts layer checkbox handlers.
   */
  updateLayerVisibility(layers: LayerVisibility): void {
    const topGroup = this.layerGroups.get('topCopper');
    const bottomGroup = this.layerGroups.get('bottomCopper');
    const viaGroup = this.layerGroups.get('vias');

    if (topGroup) topGroup.visible = layers.topCopper;
    if (bottomGroup) bottomGroup.visible = layers.bottomCopper;

    // Absent means visible, the same reading the 2D view takes.
    const innerVisible = layers.innerCopper !== false;
    for (const [key, group] of this.layerGroups) {
      if (key.startsWith('inner')) {
        group.visible = innerVisible;
      }
    }
    // Vias visible when either copper layer is visible
    if (viaGroup) viaGroup.visible = layers.topCopper || layers.bottomCopper;
  }

  /**
   * Dispose all Three.js resources and remove DOM elements.
   */
  dispose(): void {
    this.active = false;

    if (this.animFrameId !== null) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }

    window.removeEventListener('resize', this.onResize);

    // Dispose board group contents
    this.clearBoardGroup();

    // Dispose controls
    if (this.controls) {
      this.controls.dispose();
      this.controls = null;
    }

    // Dispose scene children (lights, sprites, etc.)
    if (this.scene) {
      this.scene.traverse((obj) => {
        if (obj instanceof THREE.Mesh) {
          obj.geometry?.dispose();
          if (obj.material) {
            if (Array.isArray(obj.material)) {
              obj.material.forEach(m => m.dispose());
            } else {
              obj.material.dispose();
            }
          }
        }
        if (obj instanceof THREE.Sprite) {
          const spriteMat = obj.material as THREE.SpriteMaterial;
          spriteMat.map?.dispose();
          spriteMat.dispose();
        }
      });
      this.scene.clear();
      this.scene = null;
    }

    // Remove renderer DOM element
    if (this.renderer) {
      if (this.renderer.domElement.parentElement) {
        this.renderer.domElement.parentElement.removeChild(this.renderer.domElement);
      }
      this.renderer.dispose();
      this.renderer = null;
    }

    this.camera = null;
    this.boardGroup = null;
    this.container = null;
    this.layerGroups.clear();

    // Update debug surface
    this.updateDebugSurface();

    console.log('[3D] Disposed');
  }

  /**
   * Update the scene background color from a CSS color string.
   */
  setBackground(color: string): void {
    if (this.scene) {
      this.scene.background = new THREE.Color(color);
    }
  }

  /**
   * Whether the renderer is currently active and running.
   */
  get isActive(): boolean {
    return this.active;
  }

  /**
   * Load a GLB 3D model for a component, replacing its placeholder box mesh.
   * Finds the placeholder by name `component-${refdes}`, copies its transform,
   * removes it, and adds the loaded GLTF scene at the same position/rotation.
   * Errors are logged to console — callers don't need to handle failures.
   */
  loadComponentModel(url: string, refdes: string): void {
    if (!this.boardGroup) {
      console.error(`[3D] GLB load failed for ${refdes}: no board group`);
      return;
    }

    const meshName = `component-${refdes}`;
    let placeholder: THREE.Object3D | null = null;
    this.boardGroup.traverse((obj) => {
      if (obj.name === meshName) placeholder = obj;
    });

    if (!placeholder) {
      console.error(`[3D] GLB load failed for ${refdes}: placeholder mesh "${meshName}" not found`);
      return;
    }

    const loader = new GLTFLoader();
    const boardGroup = this.boardGroup;
    const loadedModels = this.loadedModels;
    const pos = (placeholder as THREE.Mesh).position.clone();
    const rot = (placeholder as THREE.Mesh).rotation.clone();

    loader.load(
      url,
      (gltf) => {
        const model = gltf.scene;
        model.position.copy(pos);
        model.rotation.copy(rot);
        model.name = `model-${refdes}`;

        // Remove the placeholder box
        if (placeholder && placeholder.parent) {
          if (placeholder instanceof THREE.Mesh) {
            placeholder.geometry?.dispose();
            if (placeholder.material) {
              if (Array.isArray(placeholder.material)) {
                placeholder.material.forEach(m => m.dispose());
              } else {
                (placeholder.material as THREE.Material).dispose();
              }
            }
          }
          placeholder.parent.remove(placeholder);
        }

        // Add loaded model to the same parent (topGroup)
        boardGroup.traverse((obj) => {
          if (obj.name === 'layer-top') {
            obj.add(model);
          }
        });

        loadedModels.set(refdes, model);
        console.log(`[3D] GLB loaded for ${refdes}: ${url}`);
      },
      undefined,
      (error) => {
        console.error(`[3D] GLB load failed for ${refdes}: ${error}`);
      },
    );
  }

  /**
   * Load a 3D model from EasyEDA OBJ text, replacing the placeholder box mesh.
   * Parses the non-standard OBJ format, builds BufferGeometry per material group,
   * and adds the resulting Group to the scene at the placeholder's position/rotation.
   * Errors are logged to console — callers don't need to handle failures.
   */
  loadComponentFromOBJ(objText: string, refdes: string): void {
    if (!this.boardGroup) {
      console.error(`[3D] OBJ load failed for ${refdes}: no board group`);
      return;
    }

    const meshName = `component-${refdes}`;
    let placeholder: THREE.Object3D | null = null;
    this.boardGroup.traverse((obj) => {
      if (obj.name === meshName) placeholder = obj;
    });

    if (!placeholder) {
      console.error(`[3D] OBJ load failed for ${refdes}: placeholder mesh "${meshName}" not found`);
      return;
    }

    let groups;
    try {
      groups = parseEasyEdaOBJ(objText);
    } catch (error) {
      console.error(`[3D] OBJ parse failed: ${error}`);
      return;
    }

    if (groups.length === 0) {
      console.error(`[3D] OBJ parse failed: no geometry groups for ${refdes}`);
      return;
    }

    const model = new THREE.Group();
    model.name = `model-${refdes}`;

    for (const group of groups) {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute('position', new THREE.BufferAttribute(group.positions, 3));
      geo.setAttribute('normal', new THREE.BufferAttribute(group.normals, 3));

      const mat = new THREE.MeshStandardMaterial({
        color: new THREE.Color(group.materialColor.r, group.materialColor.g, group.materialColor.b),
        metalness: 0.3,
        roughness: 0.5,
        side: THREE.DoubleSide,
        transparent: group.opacity < 1.0,
        opacity: group.opacity,
      });

      const mesh = new THREE.Mesh(geo, mat);
      model.add(mesh);
    }

    // Place model at component position, on top of board.
    // Re-center model XY so its bounding box center aligns with the
    // pad centroid (placeholder position). This handles OBJ models that
    // have their origin at pin 1 rather than at the geometric center.
    const bbox = new THREE.Box3().setFromObject(model);
    const bboxCenter = new THREE.Vector3();
    bbox.getCenter(bboxCenter);

    // Shift children so model bbox center maps to (0,0,0)
    model.children.forEach((child) => {
      child.position.x -= bboxCenter.x;
      child.position.y -= bboxCenter.y;
    });

    const pos = (placeholder as THREE.Mesh).position.clone();
    const rot = (placeholder as THREE.Mesh).rotation.clone();
    model.position.set(pos.x, pos.y, BOARD_THICKNESS_MM);
    model.rotation.copy(rot);

    // Dispose and remove placeholder
    const ph = placeholder as THREE.Object3D;
    if (ph instanceof THREE.Mesh) {
      ph.geometry?.dispose();
      if (ph.material) {
        if (Array.isArray(ph.material)) {
          ph.material.forEach((m: THREE.Material) => m.dispose());
        } else {
          (ph.material as THREE.Material).dispose();
        }
      }
    }
    if (ph.parent) {
      ph.parent.remove(ph);
    }

    // Dispose previous model for same refdes if reloading
    const prev = this.loadedModels.get(refdes);
    if (prev) {
      prev.traverse((obj) => {
        if (obj instanceof THREE.Mesh) {
          obj.geometry?.dispose();
          if (obj.material) {
            if (Array.isArray(obj.material)) {
              obj.material.forEach(m => m.dispose());
            } else {
              (obj.material as THREE.Material).dispose();
            }
          }
        }
      });
      prev.parent?.remove(prev);
      this._objModelCount--;
    }

    // Add to top layer group
    this.boardGroup.traverse((obj) => {
      if (obj.name === 'layer-top') {
        obj.add(model);
      }
    });

    this.loadedModels.set(refdes, model);
    this._objModelCount++;

    console.log(`[3D] OBJ loaded for ${refdes}`);
    this.updateDebugSurface();
  }

  // -- Copper geometry builders --

  /**
   * Build merged trace geometry per layer.
   * Each trace segment → flat quad (4 verts, 2 tris) at the correct Z-height.
   * All segments on a layer are merged into one BufferGeometry for draw-call efficiency.
   */
  private buildTraces(
    traces: TraceInfo[],
    topGroup: THREE.Group,
    bottomGroup: THREE.Group,
    innerGroups: THREE.Group[] = [],
  ): void {
    const topPositions: number[] = [];
    const bottomPositions: number[] = [];
    const innerPositions: number[][] = innerGroups.map(() => []);
    let topSegCount = 0;
    let bottomSegCount = 0;
    let innerSegCount = 0;

    const CAP_SEGMENTS = 8; // semicircle resolution for round caps

    // Helper: push a round cap (disc) at a point for top and bottom faces + side wall
    function pushRoundCap(positions: number[], cx: number, cy: number, radius: number, zBot: number, zTop: number) {
      const step = Math.PI * 2 / (CAP_SEGMENTS * 2);
      for (let i = 0; i < CAP_SEGMENTS * 2; i++) {
        const a0 = i * step;
        const a1 = (i + 1) * step;
        const x0 = cx + Math.cos(a0) * radius;
        const y0 = cy + Math.sin(a0) * radius;
        const x1 = cx + Math.cos(a1) * radius;
        const y1 = cy + Math.sin(a1) * radius;

        // Top disc triangle
        positions.push(cx, cy, zTop, x0, y0, zTop, x1, y1, zTop);
        // Bottom disc triangle
        positions.push(cx, cy, zBot, x1, y1, zBot, x0, y0, zBot);
        // Side wall quad (2 triangles)
        positions.push(x0, y0, zBot, x0, y0, zTop, x1, y1, zTop);
        positions.push(x0, y0, zBot, x1, y1, zTop, x1, y1, zBot);
      }
    }

    for (const trace of traces) {
      const widthMm = trace.width * NM_TO_MM;
      const halfW = widthMm / 2;
      const innerIndex = innerLayerIndex(trace.layer) ?? -1;
      const isInner = innerIndex >= 0 && innerIndex < innerPositions.length;
      const isTop = trace.layer === 'Top';
      const positions = isInner
        ? innerPositions[innerIndex]
        : isTop
          ? topPositions
          : bottomPositions;

      // Inner copper sits inside the substrate, evenly spaced between the two
      // faces, so which layer a trace is on can be read from the side.
      const innerZ = isInner
        ? innerLayerDepth(innerIndex, innerPositions.length, BOARD_THICKNESS_MM)
        : 0;
      const zBot = isInner
        ? innerZ - COPPER_THICKNESS_MM / 2
        : isTop
          ? F_COPPER_BOT_Z
          : B_COPPER_BOT_Z;
      const zTop = isInner
        ? innerZ + COPPER_THICKNESS_MM / 2
        : isTop
          ? F_COPPER_TOP_Z
          : B_COPPER_TOP_Z;

      // Track endpoints for round caps (deduplicate shared points)
      const capPoints = new Set<string>();

      for (const seg of trace.segments) {
        const sx = seg.start_x * NM_TO_MM;
        const sy = seg.start_y * NM_TO_MM;
        const ex = seg.end_x * NM_TO_MM;
        const ey = seg.end_y * NM_TO_MM;

        const dx = ex - sx;
        const dy = ey - sy;
        const len = Math.sqrt(dx * dx + dy * dy);
        if (len < 1e-6) continue;

        const px = (-dy / len) * halfW;
        const py = (dx / len) * halfW;

        const slx = sx + px, sly = sy + py;
        const srx = sx - px, sry = sy - py;
        const erx = ex - px, ery = ey - py;
        const elx = ex + px, ely = ey + py;

        // TOP FACE
        positions.push(slx, sly, zTop, srx, sry, zTop, erx, ery, zTop);
        positions.push(slx, sly, zTop, erx, ery, zTop, elx, ely, zTop);

        // BOTTOM FACE
        positions.push(slx, sly, zBot, erx, ery, zBot, srx, sry, zBot);
        positions.push(slx, sly, zBot, elx, ely, zBot, erx, ery, zBot);

        // LEFT SIDE
        positions.push(slx, sly, zBot, slx, sly, zTop, elx, ely, zTop);
        positions.push(slx, sly, zBot, elx, ely, zTop, elx, ely, zBot);

        // RIGHT SIDE
        positions.push(srx, sry, zBot, erx, ery, zTop, srx, sry, zTop);
        positions.push(srx, sry, zBot, erx, ery, zBot, erx, ery, zTop);

        // No flat start/end caps — round caps handle endpoints

        // Register endpoints for round caps
        const sKey = `${sx.toFixed(4)},${sy.toFixed(4)}`;
        const eKey = `${ex.toFixed(4)},${ey.toFixed(4)}`;
        if (!capPoints.has(sKey)) {
          capPoints.add(sKey);
          pushRoundCap(positions, sx, sy, halfW, zBot, zTop);
        }
        if (!capPoints.has(eKey)) {
          capPoints.add(eKey);
          pushRoundCap(positions, ex, ey, halfW, zBot, zTop);
        }

        if (isInner) innerSegCount++;
        else if (isTop) topSegCount++;
        else bottomSegCount++;
      }
    }

    addCopperMesh(topPositions, LAYER_COLORS.top_copper, 'traces-top', topGroup);
    addCopperMesh(bottomPositions, LAYER_COLORS.bottom_copper, 'traces-bottom', bottomGroup);
    innerPositions.forEach((positions, index) => {
      addCopperMesh(
        positions,
        INNER_LAYER_COLORS[index % INNER_LAYER_COLORS.length],
        `traces-inner-${index + 1}`,
        innerGroups[index],
      );
    });

    this._traceSegmentCount = topSegCount + bottomSegCount + innerSegCount;
  }

  /**
   * Build solder mask — green protective layer covering copper, with openings at pads.
   *
   * Uses a grid-based approach: divides the board into cells, marks cells that
   * overlap with pads as "open", and creates green quads only for covered cells.
   * This gives the appearance of solder mask with pad openings.
   *
   * Simplified approach: render full board mask plane, then render pads ABOVE it.
   * The mask sits between copper and pads in Z-order.
   */
  private buildSolderMask(
    widthMm: number,
    heightMm: number,
    components: ComponentInfo[],
    topGroup: THREE.Group,
    bottomGroup: THREE.Group,
  ): void {
    const MASK_EXPANSION = 0.05; // 50μm expansion around pad openings

    // Collect pad rectangles (in mm, board coordinates) for masking
    const padRects: { x: number; y: number; hw: number; hh: number }[] = [];
    for (const comp of components) {
      const cx = comp.x_nm * NM_TO_MM;
      const cy = comp.y_nm * NM_TO_MM;
      const rad = ((comp.rotation_mdeg || 0) / 1000) * (Math.PI / 180);
      const cos = Math.cos(rad);
      const sin = Math.sin(rad);

      for (const pad of comp.pads) {
        const px = pad.x_nm * NM_TO_MM;
        const py = pad.y_nm * NM_TO_MM;
        const wx = cx + (px * cos - py * sin);
        const wy = cy + (px * sin + py * cos);
        const hw = (pad.width_nm * NM_TO_MM) / 2 + MASK_EXPANSION;
        const hh = (pad.height_nm * NM_TO_MM) / 2 + MASK_EXPANSION;
        padRects.push({ x: wx, y: wy, hw: Math.max(hw, hh), hh: Math.max(hw, hh) });
      }
    }

    // Grid-based mask: divide board into cells, skip cells that overlap pads
    const CELL_SIZE = 0.5; // 0.5mm grid cells
    const cols = Math.ceil(widthMm / CELL_SIZE);
    const rows = Math.ceil(heightMm / CELL_SIZE);

    const topPositions: number[] = [];
    const botPositions: number[] = [];

    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const cx = (col + 0.5) * CELL_SIZE;
        const cy = (row + 0.5) * CELL_SIZE;

        // Check if this cell overlaps any pad
        let overlaps = false;
        for (const pr of padRects) {
          if (Math.abs(cx - pr.x) < pr.hw + CELL_SIZE / 2 &&
              Math.abs(cy - pr.y) < pr.hh + CELL_SIZE / 2) {
            overlaps = true;
            break;
          }
        }
        if (overlaps) continue;

        const x0 = col * CELL_SIZE;
        const y0 = row * CELL_SIZE;
        const x1 = Math.min(x0 + CELL_SIZE, widthMm);
        const y1 = Math.min(y0 + CELL_SIZE, heightMm);

        // Top solder mask quad (sits on top of front copper)
        topPositions.push(x0, y0, F_MASK_Z, x1, y0, F_MASK_Z, x1, y1, F_MASK_Z);
        topPositions.push(x0, y0, F_MASK_Z, x1, y1, F_MASK_Z, x0, y1, F_MASK_Z);

        // Bottom solder mask quad (sits below back copper)
        botPositions.push(x0, y0, B_MASK_Z, x1, y1, B_MASK_Z, x1, y0, B_MASK_Z);
        botPositions.push(x0, y0, B_MASK_Z, x0, y1, B_MASK_Z, x1, y1, B_MASK_Z);
      }
    }

    // KiCad solder mask: semi-transparent dark green, glossy plastic
    const maskMat = new THREE.MeshPhysicalMaterial({
      color: PCB_GREEN,
      metalness: 0.0,
      roughness: 0.25,
      transparent: true,
      opacity: 0.85,
      side: THREE.DoubleSide,
      depthWrite: true,
      polygonOffset: true,     // Prevent z-fighting with copper underneath
      polygonOffsetFactor: -1,
      polygonOffsetUnits: -1,
    });

    if (topPositions.length > 0) {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute('position', new THREE.Float32BufferAttribute(topPositions, 3));
      geo.computeVertexNormals();
      const mesh = new THREE.Mesh(geo, maskMat);
      mesh.name = 'solder-mask-top';
      topGroup.add(mesh);
    }

    if (botPositions.length > 0) {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute('position', new THREE.Float32BufferAttribute(botPositions, 3));
      geo.computeVertexNormals();
      const mesh = new THREE.Mesh(geo, maskMat.clone());
      mesh.name = 'solder-mask-bottom';
      bottomGroup.add(mesh);
    }

    console.log(`[3D] Built solder mask: ${cols}x${rows} grid, ${padRects.length} pad openings`);
  }

  /**
   * Build pad geometry from components.
   * Pads are merged into per-layer BufferGeometry. Through-hole pads go in both groups.
   */
  private buildPads(components: ComponentInfo[], topGroup: THREE.Group, bottomGroup: THREE.Group): void {
    const topPositions: number[] = [];
    const bottomPositions: number[] = [];
    let padCount = 0;

    for (const comp of components) {
      const compXmm = comp.x_nm * NM_TO_MM;
      const compYmm = comp.y_nm * NM_TO_MM;
      const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
      const cosR = Math.cos(radians);
      const sinR = Math.sin(radians);

      for (const pad of comp.pads) {
        // Rotate pad offset by component rotation
        const padXmm = pad.x_nm * NM_TO_MM;
        const padYmm = pad.y_nm * NM_TO_MM;
        const rotX = padXmm * cosR - padYmm * sinR;
        const rotY = padXmm * sinR + padYmm * cosR;

        const worldX = compXmm + rotX;
        const worldY = compYmm + rotY;
        const wMm = pad.width_nm * NM_TO_MM;
        const hMm = pad.height_nm * NM_TO_MM;

        const isTop = (pad.layer_mask & LAYER_MASK.TOP_COPPER) !== 0;
        const isBottom = (pad.layer_mask & LAYER_MASK.BOTTOM_COPPER) !== 0;

        // Build pad shape as triangles based on pad.shape
        const padTris = this.buildPadTriangles(worldX, worldY, wMm, hMm, radians, pad.shape);

        if (isTop) {
          for (const tri of padTris) {
            topPositions.push(tri[0], tri[1], Z_TOP_PAD);
            topPositions.push(tri[2], tri[3], Z_TOP_PAD);
            topPositions.push(tri[4], tri[5], Z_TOP_PAD);
          }
        }
        if (isBottom) {
          for (const tri of padTris) {
            bottomPositions.push(tri[0], tri[1], Z_BOTTOM_PAD);
            bottomPositions.push(tri[2], tri[3], Z_BOTTOM_PAD);
            bottomPositions.push(tri[4], tri[5], Z_BOTTOM_PAD);
          }
        }

        padCount++;
      }
    }

    // Create pad meshes per layer
    addCopperMesh(topPositions, LAYER_COLORS.top_copper, 'pads-top', topGroup);
    addCopperMesh(bottomPositions, LAYER_COLORS.bottom_copper, 'pads-bottom', bottomGroup);

    this._padCount = padCount;

    // The holes themselves. One template cylinder scaled per drill, the way
    // the vias are done, and a board thickness plus a hair so the ends are
    // never coplanar with the faces.
    const drills = padDrills(components);
    this._padDrillCount = drills.length;
    if (drills.length > 0) {
      const template = new THREE.CylinderGeometry(
        0.5,
        0.5,
        BOARD_THICKNESS_MM + 0.02,
        16,
        1,
        false,
      );
      template.rotateX(Math.PI / 2);

      const drillMesh = new THREE.InstancedMesh(
        template,
        new THREE.MeshStandardMaterial({
          color: 0x0a0a0a, // The same dark void a via's hole is drawn with.
          metalness: 0.1,
          roughness: 0.9,
        }),
        drills.length,
      );
      drillMesh.name = 'pad-drills';

      const matrix = new THREE.Matrix4();
      for (let i = 0; i < drills.length; i++) {
        const drill = drills[i];
        // The template is a unit-diameter cylinder, so the diameter is the
        // scale. Z is left alone: a drilled hole goes through the board.
        matrix.makeScale(drill.diameter, drill.diameter, 1);
        matrix.setPosition(drill.x, drill.y, 0);
        drillMesh.setMatrixAt(i, matrix);
      }
      drillMesh.instanceMatrix.needsUpdate = true;
      topGroup.add(drillMesh);
    }

    console.log(`[3D] Built ${padCount} pads, ${drills.length} pad drills`);
  }

  /**
   * Build triangles for a single pad shape at the given world position.
   * Returns array of [x0,y0, x1,y1, x2,y2] triangles.
   */
  private buildPadTriangles(
    cx: number, cy: number,
    w: number, h: number,
    rotation: number,
    shape: string,
  ): number[][] {
    const cosR = Math.cos(rotation);
    const sinR = Math.sin(rotation);

    // Helper to rotate a point around (cx, cy)
    const rot = (lx: number, ly: number): [number, number] => {
      const rx = lx * cosR - ly * sinR;
      const ry = lx * sinR + ly * cosR;
      return [cx + rx, cy + ry];
    };

    const hw = w / 2;
    const hh = h / 2;

    switch (shape) {
      case 'circle': {
        // Approximate circle with 12-segment fan
        const segments = 12;
        const tris: number[][] = [];
        const r = Math.max(hw, hh);
        for (let i = 0; i < segments; i++) {
          const a0 = (i / segments) * Math.PI * 2;
          const a1 = ((i + 1) / segments) * Math.PI * 2;
          const [x0, y0] = rot(Math.cos(a0) * r, Math.sin(a0) * r);
          const [x1, y1] = rot(Math.cos(a1) * r, Math.sin(a1) * r);
          tris.push([cx, cy, x0, y0, x1, y1]);
        }
        return tris;
      }

      case 'roundrect':
      case 'oblong': {
        // Approximate with 4 corner arcs (4 segments each) + rect body
        // For simplicity and merge-efficiency, use an octagon approximation
        const cornerR = Math.min(hw, hh) * 0.25;
        const tris: number[][] = [];

        // 8 corners of the octagon
        const pts: [number, number][] = [
          rot(-hw + cornerR, -hh),
          rot(hw - cornerR, -hh),
          rot(hw, -hh + cornerR),
          rot(hw, hh - cornerR),
          rot(hw - cornerR, hh),
          rot(-hw + cornerR, hh),
          rot(-hw, hh - cornerR),
          rot(-hw, -hh + cornerR),
        ];

        // Fan triangulation from center
        for (let i = 0; i < pts.length; i++) {
          const [x0, y0] = pts[i];
          const [x1, y1] = pts[(i + 1) % pts.length];
          tris.push([cx, cy, x0, y0, x1, y1]);
        }
        return tris;
      }

      case 'rect':
      default: {
        // Two triangles for a rectangle
        const [tl0, tl1] = rot(-hw, -hh);
        const [tr0, tr1] = rot(hw, -hh);
        const [br0, br1] = rot(hw, hh);
        const [bl0, bl1] = rot(-hw, hh);
        return [
          [tl0, tl1, tr0, tr1, br0, br1],
          [tl0, tl1, br0, br1, bl0, bl1],
        ];
      }
    }
  }

  /**
   * Build via geometry using InstancedMesh for efficiency.
   * Vias are cylinders spanning full board thickness with a tube geometry (drilled hole).
   */
  private buildVias(vias: ViaInfo[], viaGroup: THREE.Group, innerCount = 0): void {
    if (vias.length === 0) {
      console.log('[3D] Built 0 vias (instanced)');
      return;
    }

    // Use tube geometry (cylinder with inner hole) for via annular ring
    // Template: outer_diameter, board thickness height, 16 radial segments
    const firstVia = vias[0];
    const outerR = (firstVia.outer_diameter * NM_TO_MM) / 2;
    const innerR = (firstVia.drill * NM_TO_MM) / 2;

    // For vias with varying sizes, use the most common size as template
    // For simplicity, use a single tube geometry and per-instance scale
    const tubeGeo = new THREE.CylinderGeometry(outerR, outerR, BOARD_THICKNESS_MM, 16, 1, false);
    // CylinderGeometry is Y-up by default; rotate to Z-up
    tubeGeo.rotateX(Math.PI / 2);

    const viaMat = new THREE.MeshStandardMaterial({
      color: 0xB87333,       // Same copper as traces
      metalness: 0.9,
      roughness: 0.2,
    });

    const instancedMesh = new THREE.InstancedMesh(tubeGeo, viaMat, vias.length);
    instancedMesh.name = 'vias';

    const drillGeo = new THREE.CylinderGeometry(innerR, innerR, BOARD_THICKNESS_MM + 0.02, 16, 1, false);
    drillGeo.rotateX(Math.PI / 2);

    const drillMat = new THREE.MeshStandardMaterial({
      color: 0x0a0a0a,      // Dark void (drilled hole)
      metalness: 0.1,
      roughness: 0.9,
    });

    const drillInstancedMesh = new THREE.InstancedMesh(drillGeo, drillMat, vias.length);
    drillInstancedMesh.name = 'via-drills';

    const matrix = new THREE.Matrix4();
    const refOuterR = outerR;
    const refInnerR = innerR;

    for (let i = 0; i < vias.length; i++) {
      const via = vias[i];
      const x = via.x * NM_TO_MM;
      const y = via.y * NM_TO_MM;

      // A blind or buried via stops at an inner layer. The template cylinder
      // is a board thickness tall, so the span becomes a Z scale and the
      // middle of the span becomes its position.
      const span = viaSpanDepths(
        via.start_layer ?? 'Top',
        via.end_layer ?? 'Bottom',
        innerCount,
        BOARD_THICKNESS_MM,
      );
      const spanHeight = Math.max(span.top - span.bottom, COPPER_THICKNESS_MM);
      const zScale = spanHeight / BOARD_THICKNESS_MM;
      const z = (span.top + span.bottom) / 2;

      // Scale factor relative to reference via
      const thisOuterR = (via.outer_diameter * NM_TO_MM) / 2;
      const thisInnerR = (via.drill * NM_TO_MM) / 2;
      const outerScale = refOuterR > 0 ? thisOuterR / refOuterR : 1;
      const innerScale = refInnerR > 0 ? thisInnerR / refInnerR : 1;

      // Outer annular ring
      matrix.makeScale(outerScale, outerScale, zScale);
      matrix.setPosition(x, y, z);
      instancedMesh.setMatrixAt(i, matrix);

      // Drill hole (slightly taller to punch through what it passes)
      matrix.makeScale(innerScale, innerScale, zScale);
      matrix.setPosition(x, y, z);
      drillInstancedMesh.setMatrixAt(i, matrix);
    }

    instancedMesh.instanceMatrix.needsUpdate = true;
    drillInstancedMesh.instanceMatrix.needsUpdate = true;

    viaGroup.add(instancedMesh);
    viaGroup.add(drillInstancedMesh);

    this._viaCount = vias.length;

    console.log(`[3D] Built ${vias.length} vias (instanced)`);
  }

  /**
   * Build component body geometry as colored boxes with refdes labels.
  /**
   * Auto-load 3D models for components that have a model_3d UUID.
   * Fetches OBJ from EasyEDA modules API and replaces the placeholder box.
   * Non-blocking — each model loads asynchronously.
   */
  private autoLoad3DModels(components: ComponentInfo[]): void {
    for (const comp of components) {
      if (!comp.model_3d) continue;
      if (this.loadedModels.has(comp.refdes)) continue;

      const uuid = comp.model_3d;
      const refdes = comp.refdes;

      fetch3DModelByUuid(uuid)
        .then(objText => {
          if (objText) {
            this.loadComponentFromOBJ(objText, refdes);
          }
        })
        .catch(err => {
          console.warn(`[3D] Auto-load error for ${refdes}:`, err);
        });
    }
  }

  /**
   * Build colored box meshes for each component.
  /**
   * Build silkscreen geometry from EasyEDA footprint silk shapes.
   * Silk lines rendered as flat quads slightly above the board surface.
   */
  private buildSilkscreen(components: ComponentInfo[], topGroup: THREE.Group, bottomGroup: THREE.Group): void {
    const Z_TOP_SILK = F_MASK_Z + MASK_THICKNESS_MM + 0.002; // above solder mask
    const Z_BOTTOM_SILK = B_MASK_Z - MASK_THICKNESS_MM - 0.002;
    const topPositions: number[] = [];
    const bottomPositions: number[] = [];
    let shapeCount = 0;

    for (const comp of components) {
      if (!comp.silk || comp.silk.length === 0) continue;

      const compX = comp.x_nm * NM_TO_MM;
      const compY = comp.y_nm * NM_TO_MM;
      const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
      const cosR = Math.cos(radians);
      const sinR = Math.sin(radians);

      for (const shape of comp.silk) {
        const isTop = shape.layer === 'top';
        const z = isTop ? Z_TOP_SILK : Z_BOTTOM_SILK;
        const positions = isTop ? topPositions : bottomPositions;
        const w = Math.max(shape.width * NM_TO_MM, 0.1) / 2; // half-width, min 0.1mm

        if (shape.type === 'segment') {
          const x1r = shape.x1 * NM_TO_MM;
          const y1r = shape.y1 * NM_TO_MM;
          const x2r = shape.x2 * NM_TO_MM;
          const y2r = shape.y2 * NM_TO_MM;

          // Rotate by component rotation
          const wx1 = compX + x1r * cosR - y1r * sinR;
          const wy1 = compY + x1r * sinR + y1r * cosR;
          const wx2 = compX + x2r * cosR - y2r * sinR;
          const wy2 = compY + x2r * sinR + y2r * cosR;

          // Build quad (two triangles) perpendicular to line direction
          const dx = wx2 - wx1;
          const dy = wy2 - wy1;
          const len = Math.sqrt(dx * dx + dy * dy);
          if (len < 0.001) continue;

          const nx = -dy / len * w;
          const ny = dx / len * w;

          // Triangle 1
          positions.push(wx1 + nx, wy1 + ny, z);
          positions.push(wx1 - nx, wy1 - ny, z);
          positions.push(wx2 + nx, wy2 + ny, z);
          // Triangle 2
          positions.push(wx2 + nx, wy2 + ny, z);
          positions.push(wx1 - nx, wy1 - ny, z);
          positions.push(wx2 - nx, wy2 - ny, z);

          shapeCount++;
        } else if (shape.type === 'circle') {
          // Approximate circle with line segments
          const cx = compX + (shape.cx * NM_TO_MM * cosR - shape.cy * NM_TO_MM * sinR);
          const cy = compY + (shape.cx * NM_TO_MM * sinR + shape.cy * NM_TO_MM * cosR);
          const r = shape.radius * NM_TO_MM;
          const segs = Math.max(16, Math.round(r * 8));

          for (let i = 0; i < segs; i++) {
            const a1 = (i / segs) * Math.PI * 2;
            const a2 = ((i + 1) / segs) * Math.PI * 2;
            const x1 = cx + Math.cos(a1) * r;
            const y1 = cy + Math.sin(a1) * r;
            const x2 = cx + Math.cos(a2) * r;
            const y2 = cy + Math.sin(a2) * r;

            const ddx = x2 - x1;
            const ddy = y2 - y1;
            const dlen = Math.sqrt(ddx * ddx + ddy * ddy);
            if (dlen < 0.001) continue;
            const nnx = -ddy / dlen * w;
            const nny = ddx / dlen * w;

            positions.push(x1 + nnx, y1 + nny, z);
            positions.push(x1 - nnx, y1 - nny, z);
            positions.push(x2 + nnx, y2 + nny, z);
            positions.push(x2 + nnx, y2 + nny, z);
            positions.push(x1 - nnx, y1 - nny, z);
            positions.push(x2 - nnx, y2 - nny, z);
          }
          shapeCount++;
        } else if (shape.type === 'arc') {
          const cx = compX + (shape.cx * NM_TO_MM * cosR - shape.cy * NM_TO_MM * sinR);
          const cy = compY + (shape.cx * NM_TO_MM * sinR + shape.cy * NM_TO_MM * cosR);
          const r = shape.radius * NM_TO_MM;
          const segs = Math.max(8, Math.round(r * 4));

          const startA = shape.startAngle + radians;
          const endA = shape.endAngle + radians;
          const totalAngle = endA - startA;

          for (let i = 0; i < segs; i++) {
            const a1 = startA + (i / segs) * totalAngle;
            const a2 = startA + ((i + 1) / segs) * totalAngle;
            const x1 = cx + Math.cos(a1) * r;
            const y1 = cy + Math.sin(a1) * r;
            const x2 = cx + Math.cos(a2) * r;
            const y2 = cy + Math.sin(a2) * r;

            const ddx = x2 - x1;
            const ddy = y2 - y1;
            const dlen = Math.sqrt(ddx * ddx + ddy * ddy);
            if (dlen < 0.001) continue;
            const nnx = -ddy / dlen * w;
            const nny = ddx / dlen * w;

            positions.push(x1 + nnx, y1 + nny, z);
            positions.push(x1 - nnx, y1 - nny, z);
            positions.push(x2 + nnx, y2 + nny, z);
            positions.push(x2 + nnx, y2 + nny, z);
            positions.push(x1 - nnx, y1 - nny, z);
            positions.push(x2 - nnx, y2 - nny, z);
          }
          shapeCount++;
        }
      }
    }

    // Create silk meshes
    const silkMat = new THREE.MeshBasicMaterial({ color: 0xf0f0f0, side: THREE.DoubleSide });

    if (topPositions.length > 0) {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute('position', new THREE.Float32BufferAttribute(topPositions, 3));
      geo.computeVertexNormals();
      const mesh = new THREE.Mesh(geo, silkMat);
      mesh.name = 'silk-top';
      topGroup.add(mesh);
    }
    if (bottomPositions.length > 0) {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute('position', new THREE.Float32BufferAttribute(bottomPositions, 3));
      geo.computeVertexNormals();
      const mesh = new THREE.Mesh(geo, silkMat.clone());
      mesh.name = 'silk-bottom';
      bottomGroup.add(mesh);
    }

    if (shapeCount > 0) {
      console.log(`[3D] Built ${shapeCount} silkscreen shapes`);
    }
  }

  /**
   * Build colored box meshes for each component.
   * SMD parts get 1.2mm height, THT parts get 5mm height.
   * IC packages (U/IC prefix) are dark gray, passives (R/C/L) are tan.
   */
  private buildComponents(components: ComponentInfo[], topGroup: THREE.Group): void {
    let smdCount = 0;
    let thtCount = 0;

    const icMat = new THREE.MeshStandardMaterial({
      color: 0x404040,
      metalness: 0.3,
      roughness: 0.5,
    });

    const passiveMat = new THREE.MeshStandardMaterial({
      color: 0xc2a366,
      metalness: 0.1,
      roughness: 0.6,
    });

    for (const comp of components) {
      let bodyW = comp.body_width_nm * NM_TO_MM;
      let bodyH = comp.body_height_nm * NM_TO_MM;

      // Fallback: compute bounding box from pads if body dimensions missing
      // NaN-safe: !(x > 0) catches NaN, undefined, 0, and negative
      if (!(bodyW > 0) || !(bodyH > 0)) {
        if (comp.pads.length > 0) {
          let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
          for (const pad of comp.pads) {
            const hw = (pad.width_nm * NM_TO_MM) / 2;
            const hh = (pad.height_nm * NM_TO_MM) / 2;
            const px = pad.x_nm * NM_TO_MM;
            const py = pad.y_nm * NM_TO_MM;
            minX = Math.min(minX, px - hw);
            minY = Math.min(minY, py - hh);
            maxX = Math.max(maxX, px + hw);
            maxY = Math.max(maxY, py + hh);
          }
          bodyW = maxX - minX;
          bodyH = maxY - minY;
          console.log(`[3D] Warning: component ${comp.refdes} has no body dimensions, using pad bbox fallback`);
        } else {
          continue; // No body info at all — skip
        }
      }

      // Determine if THT (any pad with drill) or SMD
      const isTHT = comp.pads.some(p => p.drill_nm != null && p.drill_nm > 0);
      const compHeight = isTHT ? THT_HEIGHT_MM : SMD_HEIGHT_MM;

      if (isTHT) thtCount++;
      else smdCount++;

      // Material: IC (U/IC prefix) = dark gray, passive (R/C/L) = tan
      const prefix = comp.refdes.replace(/[0-9]+$/, '').toUpperCase();
      const isIC = prefix === 'U' || prefix === 'IC';
      const mat = isIC ? icMat : passiveMat;

      // Create box at component position
      const geo = new THREE.BoxGeometry(bodyW, bodyH, compHeight);
      const mesh = new THREE.Mesh(geo, mat);

      // Position: component center, on top of board
      const cx = comp.x_nm * NM_TO_MM;
      const cy = comp.y_nm * NM_TO_MM;
      const cz = BOARD_THICKNESS_MM + compHeight / 2;

      mesh.position.set(cx, cy, cz);

      // Rotation around Z-axis
      const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
      mesh.rotation.z = radians;

      mesh.name = `component-${comp.refdes}`;
      topGroup.add(mesh);

      // Refdes label as sprite
      const label = this.createRefdesLabel(comp.refdes);
      label.position.set(cx, cy, BOARD_THICKNESS_MM + compHeight + 0.3);
      // Scale sprite relative to body size for readability
      const labelScale = Math.max(bodyW, bodyH) * 0.8;
      label.scale.set(labelScale, labelScale * 0.5, 1);
      topGroup.add(label);
    }

    this._componentCount = smdCount + thtCount;

    console.log(`[3D] Built ${smdCount + thtCount} component bodies (${smdCount} SMD, ${thtCount} THT)`);
  }

  /**
   * Create a sprite with refdes text rendered on a canvas texture.
   */
  private createRefdesLabel(text: string): THREE.Sprite {
    const canvas = document.createElement('canvas');
    const size = 128;
    canvas.width = size;
    canvas.height = size / 2;

    const ctx = canvas.getContext('2d')!;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // White text with slight shadow for readability
    ctx.font = 'bold 32px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    // Shadow for contrast
    ctx.shadowColor = 'rgba(0,0,0,0.8)';
    ctx.shadowBlur = 4;
    ctx.shadowOffsetX = 1;
    ctx.shadowOffsetY = 1;

    ctx.fillStyle = '#ffffff';
    ctx.fillText(text, canvas.width / 2, canvas.height / 2);

    const texture = new THREE.CanvasTexture(canvas);
    texture.minFilter = THREE.LinearFilter;
    texture.magFilter = THREE.LinearFilter;

    const spriteMat = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthTest: false,
      sizeAttenuation: true,
    });

    const sprite = new THREE.Sprite(spriteMat);
    sprite.name = `label-${text}`;
    return sprite;
  }

  // -- Private --

  private setBackgroundFromCSS(): void {
    const style = getComputedStyle(document.documentElement);
    const bgColor = style.getPropertyValue('--bg-canvas').trim() || '#1a1a2e';
    this.setBackground(bgColor);
  }

  private animate = (): void => {
    if (!this.active) return;
    this.animFrameId = requestAnimationFrame(this.animate);

    // FPS tracking
    const now = performance.now();
    this.frameCount++;
    const elapsed = now - this.lastFrameTime;
    if (elapsed >= 1000) {
      this.currentFps = Math.round((this.frameCount * 1000) / elapsed);
      this.frameCount = 0;
      this.lastFrameTime = now;
      this.updateDebugSurface();

      // Log FPS every 5 seconds
      this.fpsLogTimer++;
      if (this.fpsLogTimer % 5 === 0) {
        console.log(`[3D] FPS: ${this.currentFps}`);
      }
    }

    if (this.controls) {
      this.controls.update();
    }

    if (this.renderer && this.scene && this.camera) {
      this.renderer.render(this.scene, this.camera);
    }
  };

  private onResize(): void {
    if (!this.container || !this.renderer || !this.camera) return;

    const w = this.container.clientWidth;
    const h = this.container.clientHeight;

    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  /** Public resize — call when the container dimensions change (e.g. editor toggle). */
  resize(): void {
    this.onResize();
  }

  private clearBoardGroup(): void {
    if (!this.boardGroup) return;

    // Dispose loaded GLTF model scene graphs (nested children with separate materials)
    for (const [_refdes, model] of this.loadedModels) {
      model.traverse((obj) => {
        if (obj instanceof THREE.Mesh) {
          obj.geometry?.dispose();
          if (obj.material) {
            if (Array.isArray(obj.material)) {
              obj.material.forEach(m => {
                if (m.map) m.map.dispose();
                m.dispose();
              });
            } else {
              const mat = obj.material as THREE.MeshStandardMaterial;
              if (mat.map) mat.map.dispose();
              mat.dispose();
            }
          }
        }
      });
    }
    this.loadedModels.clear();

    this.boardGroup.traverse((obj) => {
      if (obj instanceof THREE.InstancedMesh || obj instanceof THREE.Mesh) {
        obj.geometry?.dispose();
        if (obj.material) {
          if (Array.isArray(obj.material)) {
            obj.material.forEach(m => m.dispose());
          } else {
            (obj.material as THREE.Material).dispose();
          }
        }
      }
      if (obj instanceof THREE.Sprite) {
        const spriteMat = obj.material as THREE.SpriteMaterial;
        spriteMat.map?.dispose();
        spriteMat.dispose();
      }
    });

    while (this.boardGroup.children.length > 0) {
      this.boardGroup.remove(this.boardGroup.children[0]);
    }

    // Reset geometry counts
    this._componentCount = 0;
    this._traceSegmentCount = 0;
    this._padCount = 0;
    this._viaCount = 0;
    this._padDrillCount = 0;
    this._objModelCount = 0;
  }

  private getMeshCount(): number {
    let count = 0;
    if (this.scene) {
      this.scene.traverse((obj) => {
        if (obj instanceof THREE.Mesh || obj instanceof THREE.InstancedMesh) count++;
      });
    }
    return count;
  }

  private getDrawCallCount(): number {
    if (this.renderer) {
      return this.renderer.info.render.calls;
    }
    return 0;
  }

  private updateDebugSurface(): void {
    const self = this;

    (window as any).__renderer3d = {
      get isActive() { return self.active; },
      get meshCount() { return self.getMeshCount(); },
      get drawCalls() { return self.getDrawCallCount(); },
      get fps() { return self.currentFps; },
      get componentCount() { return self._componentCount; },
      get traceSegmentCount() { return self._traceSegmentCount; },
      get padCount() { return self._padCount; },
      get viaCount() { return self._viaCount; },
      get padDrillCount() { return self._padDrillCount; },
      get objModelCount() { return self._objModelCount; },
    };
  }
}
