# CodeYourPCB - Brainstorm Notes

## Core Concept

**Problem:** Obecne narzędzia PCB (KiCad, Eagle, Altium) są GUI-first. Plik projektu to XML/binary będący skutkiem ubocznym UI. Trudne do:
- Wersjonowania w git (merge conflicts, nieczytelne diffy)
- Edycji przez AI/LLM
- Współpracy zespołowej
- Automatyzacji

**Rozwiązanie:** Code-first PCB design. Piszesz kod → generuje się płytka. Plik źródłowy jest source of truth.

## Kluczowe Decyzje Architektoniczne

### Format Pliku

**Podejście: Generatywne/Proceduralne**
- Plik to kod, nie dane
- Ten sam kod = ta sama płytka (deterministyczne)
- Git-friendly (czytelne diffy)
- LLM-friendly (może edytować jak kod)

**Warstwy abstrakcji:**
```
┌─────────────────────────────────────┐
│  WARSTWA LOGICZNA (plik źródłowy)  │
│  - komponenty i połączenia          │
│  - constraints i deklaracje         │
│  - intencje ("crosstalk sensitive") │
└─────────────────┬───────────────────┘
                  │ solver/autorouter
                  ▼
┌─────────────────────────────────────┐
│  WARSTWA GEOMETRYCZNA (wyliczona)  │
│  - konkretne współrzędne            │
│  - wygenerowane ścieżki             │
│  - output: Gerber, BOM, etc.        │
└─────────────────────────────────────┘
```

### Routing Philosophy

- `connect(A -> B)` = autorouting (domyślnie)
- Każdy dodatkowy parametr = constraint dla routera
- Im więcej deklarujesz, tym mądrzejszy design

```
# minimum - działa
connect(u1.pin3 -> u2.pin7)

# z constraints
connect(u1.clk -> u2.clk) {
  layer: TOP
  min_width: 0.3mm
  crosstalk_sensitive: true
}

# pełna deklaracja
connect(u1.clk -> u2.clk) {
  signal_type: clock
  frequency: 100MHz
  rise_time: 2ns
  load_capacitance: 15pF
}
```

### Inteligencja Elektryczna (nie tylko geometryczna)

Tradycyjny DRC: "czy jest 10mil odstępu?"
Nasz system: "te sygnały mogą iść razem bo low voltage GPIO, te muszą być osobno bo crosstalk-sensitive"

**Emergentne zachowania:**
- 30 sygnałów bez przeciwwskazań → automatycznie bus
- System sam odkrywa optymalizacje na podstawie reguł i wiedzy elektrycznej

## Struktura Projektu

```
my_pcb_project/
├── board.pcb           # główny plik designu
├── constraints.pcb     # globalne reguły DRC
├── config.pcb          # ustawienia projektu
├── footprints/         # lokalne footprinty
├── models/             # 3D models
├── modules/            # reużywalne moduły
│   ├── power.pcb
│   ├── mcu.pcb
│   └── comms.pcb
├── tests/              # testy designu
│   └── design.test.pcb
└── libs/               # zewnętrzne zależności
```

## Syntax Examples

### Basic Component Placement

```
board(size=[50mm, 30mm], layers=2)

# komponenty
led1 = LED_0805(position=center.offset(x=-10mm))
r1 = Resistor_0603(value=330R, position=center.offset(x=10mm))

# połączenia
connect(led1.anode -> r1.pad1)
connect(r1.pad2 -> power.vcc)
connect(led1.cathode -> power.gnd)

# constraints
rule(min_trace_width=0.2mm)
rule(min_clearance=0.15mm)
```

### Moduły i Reużywalność

```
import power_supply from "./modules/power.pcb"
import mcu_section from "./modules/stm32_minimal.pcb"

# instancje
psu = power_supply(input=12V, output=3.3V)
mcu = mcu_section(crystal=8MHz)

# łączysz moduły
connect(psu.vout -> mcu.vcc)
connect(psu.gnd -> mcu.gnd)

# placement modułów
place(psu, region=bottom_left)
place(mcu, region=center)
```

### Power Delivery

```
power_net VCC_3V3 {
  voltage: 3.3V
  max_current: 2A
  decoupling: auto  # system sam dodaje kondensatory
}

power_plane(net=VCC_3V3, layer=INNER1)
power_plane(net=GND, layer=INNER2)
```

### Differential Pairs & High-Speed

```
diff_pair USB {
  positive: USB_DP
  negative: USB_DN
  impedance: 90ohm
  max_skew: 0.1mm
}

bus DDR_DATA [0..7] {
  length_match: true
  tolerance: 2mm
  spacing: 0.15mm
}
```

### Testy PCB

```
test "power_integrity" {
  assert trace_width(VCC_NET) >= 0.5mm
  assert distance(analog_section, digital_section) >= 10mm
  assert all_pins_connected(U1)
}

test "high_speed_compliance" {
  assert differential_pair(USB_DP, USB_DN).spacing == 90ohm_impedance
  assert length_match(DDR_DATA_BUS, tolerance=5mm)
}
```

## Widoki / Output

**Widoki (web-based, portable to standalone):**
1. Schematic view
2. Top board view (2D PCB)
3. 3D view

**Output (progressive):**
- MVP: top board view render
- Later: Gerber files
- Later: BOM (bill of materials)
- Later: Pick and place files
- Later: 3D export
- Later: Netlist do symulacji

## Biblioteki Komponentów

**Podejście:** Import z istniejących bibliotek (KiCad etc.) + możliwość proceduralnego generowania:

```
# import
from kicad_libs import Resistor_0603, LED_0805

# proceduralne
my_qfp = QFP(pins=48, pitch=0.5mm, body=7x7mm)
```

## Tooling Ideas

### Visual Diff
- Integracja z git - pokazuje zmiany na renderze płytki
- "Było tu → jest tu" wizualnie, nie tylko w kodzie

### CI/CD dla PCB
- Każdy commit odpala testy
- Czerwone/zielone jak w software
- Automatyczna walidacja DRC

### LLM Integration
- Claude może edytować pliki .pcb
- Mówisz: "przesuń te ścieżki dalej od analog section"
- Claude rozumie format, edytuje constraints/placement

## Open Questions

1. **Autorouter** - wbudowany czy zewnętrzny (FreeRouting)?
2. **Dokładność/Grid** - jak reprezentować precyzję (mils, mm, μm)?
3. **Format pliku** - własny DSL? Coś istniejącego (KDL, TOML)?
4. **Renderer** - Canvas? WebGL? SVG?

## Parametryzowane Projekty (Templates)

```
board MyController(mcu_type, power_rating, form_factor) {

  mcu = mcu_type(position=center)

  if power_rating > 5A {
    psu = heavy_duty_supply(rating=power_rating)
    copper_weight = 2oz
  } else {
    psu = standard_supply(rating=power_rating)
    copper_weight = 1oz
  }

  board_outline = form_factor.outline
}

# Generujesz warianty
variant_a = MyController(STM32F4, 3A, Arduino_Uno_Form)
variant_b = MyController(STM32H7, 10A, Custom_100x80mm)
```

## AI Annotations

```
# @ai-hint: this is the analog section, keep digital signals away
analog_section = region(x=0..30mm, y=0..50mm) {
  adc = ADC_IC(position=center)

  # @ai-hint: these are sensitive measurement inputs
  connect(input_header.ch1 -> adc.in1)
  connect(input_header.ch2 -> adc.in2)
}
```

## Stackup Definition

```
stackup {
  layer TOP    { type: signal, copper: 1oz }
  layer GND    { type: plane, copper: 1oz, net: GND }
  layer PWR    { type: plane, copper: 1oz, net: VCC }
  layer BOTTOM { type: signal, copper: 1oz }
}

# lub predefiniowane
stackup = standard_4layer(signal=[TOP, BOTTOM], planes=[GND, VCC])
```

## Zones and Keepouts

```
zone no_components {
  region: rectangle(0, 0, 10mm, 10mm)
  reason: "mounting hole area"
}

zone high_voltage {
  region: polygon([...])
  rules: {
    clearance: 2mm
    no_ground_pour: true
  }
  # @ai-hint: mains voltage, serious isolation required
}
```

## Electrical Rules as Code

```
electrical_rules {
  # impedance control
  rule controlled_impedance {
    applies_to: signals.where(type=high_speed)
    target: 50ohm ± 10%
  }

  # current capacity
  rule power_traces {
    applies_to: nets.where(max_current > 1A)
    min_width: current * 0.5mm_per_amp
  }

  # crosstalk
  rule sensitive_spacing {
    applies_to: signals.where(crosstalk_sensitive=true)
    min_spacing: 3 * trace_width
  }
}
```

## UI Feedback Loop (Future, not MVP)

Endgame: bidirectional sync
- Przeciągasz ścieżkę w UI → zmienia się w pliku źródłowym
- Edytujesz kod → UI się aktualizuje

MVP: code-first, UI to tylko viewer

## Kalkulacje i Symulacje

### Architektura Warstwowa

```
┌─────────────────────────────────────────────┐
│  WARSTWA 1: Wbudowane kalkulatory           │
│  - Trace width dla danego prądu             │
│  - Impedancja microstrip/stripline          │
│  - Via current capacity                     │
│  - Thermal relief                           │
│  - Podstawowe EMI spacing                   │
└─────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────┐
│  WARSTWA 2: Integracja z ngspice            │
│  - Pełna symulacja obwodu                   │
│  - Transient analysis                       │
│  - AC/DC analysis                           │
│  - Monte Carlo dla tolerancji              │
└─────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────┐
│  WARSTWA 3: EM Simulation (future)          │
│  - Signal integrity                         │
│  - Crosstalk simulation                     │
│  - EMC compliance                           │
└─────────────────────────────────────────────┘
```

### Open Source Tools do Integracji

| Narzędzie | Co robi | Licencja |
|-----------|---------|----------|
| **ngspice** | SPICE simulator (jak LTSpice) | BSD |
| **OpenEMS** | Electromagnetic field solver | GPL |
| **QUCS-S** | Circuit simulator z GUI | GPL |
| **Xyce** | Parallel circuit sim (Sandia Labs) | GPL |

### Wbudowane Formuły (must have)

1. **Trace width vs current** (IPC-2221):
   - I = k × ΔT^b × A^c
   - Gdzie A = cross-section area

2. **Microstrip impedance:**
   - Z₀ = (87 / √(εᵣ + 1.41)) × ln(5.98h / (0.8w + t))

3. **Crosstalk:**
   - Near-end: Vne/Vagg = (1/4)(Lm/L + Cm/C)
   - Far-end: Vfe = (Lm/L - Cm/C) × Tr/2Td

4. **Thermal:**
   - ΔT = P × Rth
   - Gdzie Rth zależy od copper area, vias, planes

### Użycie w kodzie

```
connect(vcc -> load) {
  max_current: 3A
  # system automatycznie:
  # - liczy min trace width (IPC-2221)
  # - sprawdza czy via'y udźwigną
  # - sugeruje copper weight
}

simulate {
  type: transient
  circuit: power_supply_section
  duration: 10ms

  check {
    ripple(VCC_3V3) < 50mV
    startup_time < 5ms
  }
}
```

## Typowe Układy (Circuit Patterns)

### Power Supply Section

```
module linear_regulator(input_voltage, output_voltage, max_current) {

  reg = LDO(model=AMS1117, variant=output_voltage)

  # input capacitors
  c_in1 = Cap_Ceramic(value=10uF, voltage_rating=input_voltage * 1.5)
  c_in2 = Cap_Ceramic(value=100nF)  # high frequency

  # output capacitors
  c_out1 = Cap_Ceramic(value=22uF)
  c_out2 = Cap_Ceramic(value=100nF)

  # placement: input caps close to input, output caps close to output
  place_group([c_in1, c_in2], near=reg.vin, max_distance=3mm)
  place_group([c_out1, c_out2], near=reg.vout, max_distance=2mm)

  # thermal
  thermal_via_array(under=reg, count=9, pattern=3x3)

  # @ai-hint: GND plane essential under this section
  require_plane(GND, under=this.bounds)
}
```

### MCU Decoupling

```
module mcu_decoupling(mcu_component) {

  # każdy pin VCC dostaje swój kondensator
  for vcc_pin in mcu_component.pins.where(net=VCC) {
    cap = Cap_0402(value=100nF)
    place(cap, near=vcc_pin, max_distance=2mm)
    connect(vcc_pin -> cap.p1)
    connect(cap.p2 -> GND)

    # @ai-hint: shortest path to GND plane via
    via(cap.p2, to_layer=GND_PLANE, max_distance=0.5mm)
  }

  # bulk capacitor
  bulk = Cap_Ceramic(value=10uF)
  place(bulk, near=mcu_component, max_distance=5mm)
}
```

### Crystal Oscillator

```
module crystal_circuit(mcu, frequency) {

  xtal = Crystal(freq=frequency)
  c_load1 = Cap_0402(value=calculate_load_cap(xtal, mcu))
  c_load2 = Cap_0402(value=calculate_load_cap(xtal, mcu))

  # placement: VERY close to MCU
  place_group([xtal, c_load1, c_load2],
    near=mcu.pins.osc_in,
    max_distance=3mm)

  # routing
  connect(mcu.osc_in -> xtal.in) { max_length: 5mm }
  connect(mcu.osc_out -> xtal.out) { max_length: 5mm }
  connect(xtal.in -> c_load1 -> GND)
  connect(xtal.out -> c_load2 -> GND)

  # @ai-hint: NO traces under crystal, guard ring recommended
  zone crystal_keepout {
    region: xtal.bounds.expand(2mm)
    rules: { no_signals: true, ground_pour: true }
  }
}
```

### USB Interface

```
module usb_interface(type=USB_C) {

  conn = USB_Connector(type=type)
  esd = ESD_Protection(channels=2)  # for D+/D-

  # diff pair z controlled impedance
  diff_pair USB_DATA {
    positive: conn.DP -> esd.ch1 -> mcu.usb_dp
    negative: conn.DN -> esd.ch2 -> mcu.usb_dn
    impedance: 90ohm_differential
    max_skew: 0.1mm
    max_length: 50mm
  }

  # ESD close to connector
  place(esd, near=conn, max_distance=5mm)

  # CC resistors for USB-C
  if type == USB_C {
    r_cc1 = Resistor_0402(value=5.1k)
    r_cc2 = Resistor_0402(value=5.1k)
    connect(conn.cc1 -> r_cc1 -> GND)
    connect(conn.cc2 -> r_cc2 -> GND)
  }
}
```

### I2C Bus

```
module i2c_bus(master, slaves[], speed=400kHz) {

  # pull-up resistors
  r_sda = Resistor_0402(value=calculate_i2c_pullup(speed, slaves.count))
  r_scl = Resistor_0402(value=calculate_i2c_pullup(speed, slaves.count))

  place_group([r_sda, r_scl], near=master)

  # bus topology
  connect(master.sda -> r_sda -> VCC)
  connect(master.scl -> r_scl -> VCC)

  for slave in slaves {
    connect(master.sda -> slave.sda) {
      bus_topology: true  # router wie że to bus
    }
    connect(master.scl -> slave.scl) {
      bus_topology: true
    }
  }

  # @ai-hint: I2C tolerates longer traces, not timing critical
}
```

### SPI Bus

```
module spi_bus(master, slaves[], speed) {

  # shared lines
  for slave in slaves {
    connect(master.mosi -> slave.mosi) { bus: true }
    connect(master.miso -> slave.miso) { bus: true }
    connect(master.sck -> slave.sck) { bus: true }
  }

  # individual chip selects
  for i, slave in enumerate(slaves) {
    connect(master.cs[i] -> slave.cs)
  }

  if speed > 10MHz {
    # @ai-hint: high speed SPI, length matching recommended
    length_match_group([mosi, miso, sck], tolerance=5mm)
  }
}
```

## Filozofia Rozwoju

**Progressive Enhancement:**
- Minimum: połączenia działają z autorouting
- Lepiej: dodajesz deklaracje sygnałów
- Najlepiej: pełne symulacje elektryczne

**Trenowanie/Learning:**
- System uczy się z iteracji
- Optymalizuje routing na podstawie doświadczenia
- Potencjalnie training na istniejących designach z sieci

## Tech Stack Research (2025)

### Core Language: **Rust**

**Dlaczego Rust:**
- **Performance:** Na równi z C++, w WebAssembly 9% szybszy od C++ dla recursive numeric calculations
- **WebAssembly:** Kompiluje do WASM szybciej niż C++, produkuje mniejsze binaria
- **Memory safety:** 70% mniej memory errors w real-world applications vs C++
- **Longevity:** Stabilny, rosnący ekosystem, używany przez Mozilla, Google, Microsoft, Amazon
- **Production:** Figma (3x szybsze ładowanie po przejściu na WASM), Cloudflare, Discord

**Źródła:**
- [WebAssembly 3.0 Rust vs C++ Benchmarks 2025](https://markaicode.com/webassembly-3-performance-rust-cpp-benchmarks-2025/)
- [Rust WebAssembly Performance](https://byteiota.com/rust-webassembly-performance-8-10x-faster-2025-benchmarks/)
- [JetBrains Rust vs C++ 2026](https://blog.jetbrains.com/rust/2025/12/16/rust-vs-cpp-comparison-for-2026/)

---

### Desktop App Framework: **Tauri 2.0**

**Dlaczego Tauri:**
- **Memory:** 30-40 MB idle vs Electron 200-300 MB (50% mniej RAM)
- **Bundle size:** <10 MB vs Electron 80-150 MB
- **Startup:** <500ms vs Electron 1-2 seconds
- **Native:** Używa system WebView, nie bundluje Chromium
- **Rust backend:** Idealnie integruje się z core w Rust
- **2025 trend:** Adoption +35% year-over-year po Tauri 2.0

**Źródła:**
- [Tauri vs Electron 2025 Comparison](https://codeology.co.nz/articles/tauri-vs-electron-2025-desktop-development.html)
- [Tauri vs Electron Real World](https://www.levminer.com/blog/tauri-vs-electron)
- [GetHopp Tauri vs Electron](https://www.gethopp.app/blog/tauri-vs-electron)

---

### 2D Rendering: **WebGPU via wgpu** (primary) + **Canvas fallback**

**Strategia warstwowa:**

| Warstwa | Technologia | Use case |
|---------|-------------|----------|
| Primary | wgpu (WebGPU) | High-performance, 10M+ points at 45+ FPS |
| Fallback | Canvas 2D | Simple views, compatibility |

**wgpu advantages:**
- Cross-platform: Vulkan, Metal, DX12, OpenGL ES, WebGPU, WebGL2
- Pure Rust, no unsafe code
- Used by Firefox, Servo, Deno
- GPGPU compute dla autorouting
- Single codebase for all platforms

**Canvas for:**
- MVP/quick iteration
- Simple schematic views
- Older browser fallback

**Źródła:**
- [wgpu.rs](https://wgpu.rs/)
- [WebGL vs Canvas Benchmarks](https://digitaladblog.com/2025/05/21/comparing-canvas-vs-webgl-for-javascript-chart-performance/)
- [High Performance GPGPU with Rust and wgpu](https://dev.to/jaysmito101/high-performance-gpgpu-with-rust-and-wgpu-4l9i)

---

### 3D Rendering: **Three.js** (web) / **wgpu custom** (native)

**Three.js for web 3D view:**
- Lightweight (168 kB gzipped)
- Huge ecosystem, well documented
- Good for CAD/engineering visualization
- WebGPU support coming

**Alternative consideration: Babylon.js**
- Better for complex scenes (auto-culling, physics)
- Larger bundle (1.4 MB)
- Better WebGPU support currently

**Recommendation:** Start with Three.js for lighter bundle, switch to Babylon.js if complexity demands it.

**Źródła:**
- [Three.js vs Babylon.js Technical Comparison](https://dev.to/devin-rosario/babylonjs-vs-threejs-the-360deg-technical-comparison-for-production-workloads-2fn6)
- [Web 3D Performance Guide](https://modelfy.art/blog/web-3d-performance-guide)

---

### Parser / DSL: **Tree-sitter**

**Dlaczego Tree-sitter:**
- **Incremental parsing:** Tylko zmienione fragmenty są re-parsowane
- **Error tolerance:** Parser działa nawet z błędami składni
- **Fast:** Designed for parsing on every keystroke
- **Battle-tested:** GitHub, Neovim, Zed, Helix
- **Rust native:** Core w Rust, generuje parsery w C/WASM
- **LSP-ready:** Idealne dla IDE features (syntax highlighting, go-to-definition)

**Proces:**
1. Write grammar in JavaScript
2. Tree-sitter generates C parser
3. Compile to WASM for browser
4. Use Rust bindings for native

**Źródła:**
- [Tree-sitter GitHub](https://github.com/tree-sitter/tree-sitter)
- [Tree-sitter Revolution](https://www.deusinmachina.net/p/tree-sitter-revolutionizing-parsing)
- [Incremental Parsing Using Tree-sitter](https://tomassetti.me/incremental-parsing-using-tree-sitter/)

---

### File Format: **Custom DSL** (inspired by KDL)

**Kryteria:**
- Human readable/writable
- Git-friendly (meaningful diffs)
- LLM-friendly (easy to edit)
- Node-based (not just key-value)

**KDL inspiration:**
- Node-based like XML, readable like YAML
- Supports comments
- Clean syntax
- Not widely adopted yet (risk)

**Decision:** Custom DSL z Tree-sitter parserem. Syntax inspired by KDL but tailored for PCB domain.

```
board "MyProject" {
  size 100mm 80mm
  layers 4

  component U1 "STM32F4" {
    position 50mm 40mm
    rotation 0deg
  }

  connect U1.VCC -> VCC_3V3 {
    width 0.5mm
    // @ai-hint: power trace, needs thermal relief
  }
}
```

**Źródła:**
- [KDL Document Language](https://kdl.dev/)
- [Most Elegant Configuration Language](https://chshersh.com/blog/2025-01-06-the-most-elegant-configuration-language.html)

---

### Autorouter: **Custom + FreeRouting integration**

**Strategy:**

| Phase | Approach |
|-------|----------|
| MVP | FreeRouting via DSN export/import |
| V2 | Custom A* / Lee algorithm in Rust |
| V3 | GPU-accelerated (wgpu compute shaders) |

**FreeRouting:**
- Open source, proven
- Specctra DSN format (standard)
- Java-based but works well

**Future: OrthoRoute inspiration**
- GPU-accelerated routing
- PathFinder algorithm
- 41 hours vs months for complex boards

**Źródła:**
- [FreeRouting KiCad 2025](https://www.novapcba.com/2025/08/12/hv-pcb-freerouting-modernizes-kicad-autorouting-2025/)
- [OrthoRoute GPU Autorouter](https://bbenchoff.github.io/pages/OrthoRoute.html)

---

### Constraint Solver: **Z3** (complex) + **Custom** (simple)

**Z3 for:**
- Complex placement constraints
- DRC verification
- Boolean satisfiability (SAT)

**Custom for:**
- Simple geometric constraints
- Real-time validation
- Performance-critical paths

**good_lp for linear programming:**
- Route optimization
- Multiple backend options (HiGHS, SCIP)
- Pure Rust option (microlp) compiles to WASM

**Źródła:**
- [Z3 Rust bindings](https://docs.rs/z3/)
- [good_lp](https://github.com/rust-or/good_lp)

---

### Simulation: **ngspice** (shared library)

**Integration approach:**
- ngspice compiled as shared library (.so/.dll)
- Rust FFI bindings
- Generate netlist → run simulation → parse results

**API:**
- Well documented (chapter 19 of manual)
- Used by KiCad, EAGLE, Altium
- BSD-3-Clause license

**Python for prototyping:**
- ngspicepy, InSpice (2025)
- Quick iteration on simulation features

**Źródła:**
- [ngspice Shared Library](https://ngspice.sourceforge.io/shared.html)
- [InSpice PyPI](https://pypi.org/project/InSpice/)

---

### Computational Geometry: **geo-rs ecosystem** + **CGAL bindings if needed**

**Pure Rust:**
- `geo` - basic geometry types
- `geo-types` - common types
- `delaunay` - triangulation
- `rstar` - R-tree spatial indexing

**If more needed:**
- `sfcgal-rs` - CGAL bindings
- `rcgal` - Rust native (early development)

**Źródła:**
- [Rust Geo Crate](https://lib.rs/crates/geo)
- [sfcgal-rs](https://mthh.github.io/sfcgal-rs/sfcgal/)

---

### Full Stack Summary

```
┌─────────────────────────────────────────────────────────────┐
│                        FRONTEND                             │
├─────────────────────────────────────────────────────────────┤
│  Web UI          │ TypeScript + Solid.js/Svelte            │
│  2D Rendering    │ wgpu (WebGPU) + Canvas fallback         │
│  3D Rendering    │ Three.js                                 │
│  Desktop Shell   │ Tauri 2.0                                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                         CORE (Rust)                         │
├─────────────────────────────────────────────────────────────┤
│  Parser          │ Tree-sitter (custom grammar)            │
│  Data Model      │ Rust structs, serde                     │
│  Geometry        │ geo-rs, custom PCB primitives           │
│  Constraints     │ Z3 bindings + custom solver             │
│  Autorouter      │ FreeRouting (MVP) → custom (V2)         │
│  Simulation      │ ngspice FFI                             │
│  Export          │ Gerber, BOM, Pick&Place generators      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        OUTPUT                               │
├─────────────────────────────────────────────────────────────┤
│  Formats         │ Gerber, Excellon, ODB++, IPC-2581       │
│  BOM             │ CSV, JSON, Excel                        │
│  3D              │ STEP, STL                               │
│  Documentation   │ PDF, SVG                                │
└─────────────────────────────────────────────────────────────┘
```

### Technology Longevity Assessment

| Technology | Est. Longevity | Risk | Notes |
|------------|----------------|------|-------|
| Rust | 30+ years | Low | Mozilla, Google, Microsoft backing |
| WebAssembly | 30+ years | Low | W3C standard, all browsers |
| WebGPU | 20+ years | Low | W3C standard, successor to WebGL |
| Tauri | 10+ years | Medium | Active development, Rust ecosystem |
| Tree-sitter | 15+ years | Low | GitHub backing, wide adoption |
| ngspice | 20+ years | Low | Berkeley SPICE heritage, industry standard |
| Three.js | 10+ years | Low | Massive ecosystem, stable |

---

## DSL Design Best Practices

### Core Principles

**1. Start Small, Iterate:**
- Begin with minimal viable DSL solving common problems
- Evolve based on real user feedback
- Prevents over-engineering

**2. Syntax Reflects Domain:**
- Use vocabulary and concepts from PCB/electronics domain
- Reduce syntactic noise (extraneous characters)
- Ruby-like expressiveness over Java-like verbosity

**3. Collaboration with Domain Experts:**
- Listen to how PCB engineers actually work
- Establish feedback loops
- "Acting as all-mighty-expert is the single best way to create a failed DSL"

**4. Versioning from Day One:**
- Design for backward compatibility
- Feature flags for gradual introduction
- Compatibility modes for older syntax

### External vs Internal DSL

| Type | Pros | Cons |
|------|------|------|
| External (custom parser) | Full control, optimal syntax | More work, need own tooling |
| Internal (embedded in Rust) | Free tooling, type safety | Limited by host syntax |

**Decision:** External DSL with Tree-sitter parser - full control over syntax, optimized for LLM editing.

### Tooling Requirements

- Syntax highlighting (Tree-sitter queries)
- LSP server for IDE features
- Error recovery (parse partial/invalid files)
- Incremental parsing (fast on edits)

**Sources:**
- [Martin Fowler DSL Guide](https://martinfowler.com/dsl.html)
- [DSL Best Practices](https://dsls.dev/article/Best_practices_for_designing_and_implementing_DSLs.html)
- [Strumenta Complete DSL Guide](https://tomassetti.me/domain-specific-languages/)

---

## KiCad Architecture Insights

### Application Structure

```
┌─────────────────────────────────────────────────┐
│                 KiCad Project Manager           │
└─────────────────────┬───────────────────────────┘
                      │ spawns processes
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
┌───────────┐   ┌───────────┐   ┌───────────┐
│ Eeschema  │   │  Pcbnew   │   │  Other    │
│(schematic)│   │   (PCB)   │   │  tools    │
└───────────┘   └───────────┘   └───────────┘
```

**Key learnings:**
- Standalone applications, minimal coupling
- Communication via WxEvents (Kiway interconnect)
- Single-threaded GUI + processing (simpler, synchronous)

### PCB Editor Internals

**Commit System:**
- Changes staged in "Commit" objects
- GUI/state updated only when commit is pushed
- Enables atomic undo/redo operations

**Precision:**
- Nanometer internal resolution (signed 32-bit)
- Max dimension: ~2.14 meters
- Up to 32 copper + 32 technical layers

### File Format (S-Expression)

```lisp
(footprint "NAME"
  (version VERSION)
  (generator GENERATOR)
  (layer "F.Cu")
  (pad "1" smd rect
    (at 0 0)
    (size 1.5 1)
    (layers "F.Cu" "F.Paste" "F.Mask")
  )
)
```

**Key points:**
- Token: `(keyword value-list)`
- Free-format, keywords lowercase
- Nested arbitrarily deep
- No whitespace after opening paren

**Sources:**
- [KiCad File Formats](https://dev-docs.kicad.org/en/file-formats/index.html)
- [KiCad S-Expression Intro](https://dev-docs.kicad.org/en/file-formats/sexpr-intro/index.html)
- [KiCad Architecture Forum](https://forum.kicad.info/t/kicad-software-architecture/27949)

---

## Plugin System Architecture

### Approaches Comparison

| Approach | Security | Performance | Complexity | Cross-lang |
|----------|----------|-------------|------------|------------|
| WASM plugins | High (sandboxed) | Medium | Medium | Yes |
| Dynamic loading | Low (unsafe) | High | High | No |
| gRPC/IPC | High | Low | Medium | Yes |

### Recommendation: WASM Plugins

**Why WASM:**
- Sandboxed by default (safe to run untrusted code)
- Cross-language (plugins in Rust, Go, Python, etc.)
- Portable (works in browser and native)
- Fine-grained resource management

**Libraries:**
- **plugy** - Rust WASM plugin framework
- **wasmtime** - Standalone WASM runtime
- **wasmer** - Embedding-focused WASM runtime

**Real-world examples:**
- Zellij (terminal multiplexer) - WASM plugins
- Veloren (game) - WASM plugins
- Figma - uses WASM for plugins

**Plugin interface pattern:**

```rust
#[plugy::plugin]
trait PcbPlugin {
    fn name(&self) -> String;
    fn on_component_placed(&mut self, component: &Component);
    fn on_route_completed(&mut self, route: &Route);
    fn custom_drc_check(&self, board: &Board) -> Vec<Violation>;
}
```

**Sources:**
- [Rust Plugin Systems Blog](https://blog.anirudha.dev/rust-plugin-system)
- [NullDeref Plugin Series](https://nullderef.com/series/rust-plugins/)
- [Plugy Crate](https://docs.rs/plugy/latest/plugy/)

---

## Language Server Protocol (LSP)

### Implementation Stack

```
┌─────────────────────────────────────────────────┐
│              IDE / Editor                       │
└─────────────────────┬───────────────────────────┘
                      │ JSON-RPC (stdio/TCP)
                      ▼
┌─────────────────────────────────────────────────┐
│           tower-lsp / tower-lsp-server          │
│         (async Rust LSP framework)              │
└─────────────────────┬───────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────┐
│              Our PCB Language Server            │
│  - Tree-sitter parser                           │
│  - Semantic analysis                            │
│  - Diagnostics (DRC errors as LSP diagnostics)  │
└─────────────────────────────────────────────────┘
```

### Key LSP Features to Implement

| Feature | Priority | Description |
|---------|----------|-------------|
| `textDocument/diagnostic` | P0 | DRC errors, syntax errors |
| `textDocument/hover` | P0 | Component info, net details |
| `textDocument/completion` | P0 | Component names, pin names |
| `textDocument/definition` | P1 | Go to component/net definition |
| `textDocument/references` | P1 | Find all uses of net/component |
| `textDocument/formatting` | P2 | Auto-format PCB code |
| `textDocument/codeAction` | P2 | Quick fixes for DRC violations |

### Libraries

- **tower-lsp** - Async LSP framework
- **lsp-types** - Standard LSP type definitions
- **tower-lsp-boilerplate** - Project template

**Sources:**
- [tower-lsp GitHub](https://github.com/ebkalderon/tower-lsp)
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)

---

## Undo/Redo System

### Command Pattern (Recommended)

```rust
trait Command {
    fn execute(&mut self, board: &mut Board);
    fn undo(&mut self, board: &mut Board);
    fn description(&self) -> String;
}

struct MoveComponentCommand {
    component_id: ComponentId,
    from: Point,
    to: Point,
}

impl Command for MoveComponentCommand {
    fn execute(&mut self, board: &mut Board) {
        board.move_component(self.component_id, self.to);
    }

    fn undo(&mut self, board: &mut Board) {
        board.move_component(self.component_id, self.from);
    }
}
```

### Command Processor

```rust
struct CommandProcessor {
    history: Vec<Box<dyn Command>>,
    undo_stack: Vec<Box<dyn Command>>,
    max_history: usize,
}

impl CommandProcessor {
    fn execute(&mut self, cmd: Box<dyn Command>, board: &mut Board) {
        cmd.execute(board);
        self.history.push(cmd);
        self.undo_stack.clear(); // Clear redo stack
    }

    fn undo(&mut self, board: &mut Board) { ... }
    fn redo(&mut self, board: &mut Board) { ... }
}
```

### CAD-Specific Challenges

1. **Large object models** - Don't store entire model per operation
2. **Pointer consistency** - Use IDs instead of pointers
3. **Composite operations** - Group related changes
4. **Collaborative editing** - Track site IDs, state vectors

**Sources:**
- [Command Pattern Undo/Redo](https://gernotklingler.com/blog/implementing-undoredo-with-the-command-pattern/)
- [CAD Undo/Redo Research](https://www.sciencedirect.com/science/article/pii/S2288430014500164)

---

## Spatial Indexing

### Libraries

| Library | Structure | Use Case |
|---------|-----------|----------|
| **rstar** | R*-tree | General spatial queries |
| **spatialtree** | Quadtree/Octree | Realtime, batch inserts |

### R*-tree vs Quadtree

| Aspect | R*-tree | Quadtree |
|--------|---------|----------|
| Query performance | Better for ranges | Better for point queries |
| Insert performance | O(log n) | O(log n) |
| Memory | More compact | Fixed node size |
| Best for | Mixed shapes, overlaps | Regular grids, sparse data |

**Recommendation:** R*-tree (rstar) for PCB - mixed shapes, need range queries for DRC.

### Usage Pattern

```rust
use rstar::{RTree, AABB};

// Insert components
let mut tree: RTree<Component> = RTree::new();
for component in components {
    tree.insert(component);
}

// Query components in area
let query_box = AABB::from_corners([0.0, 0.0], [10.0, 10.0]);
for component in tree.locate_in_envelope(&query_box) {
    // Check DRC
}

// Find nearest component
let nearest = tree.nearest_neighbor(&point);
```

**Sources:**
- [rstar GitHub](https://github.com/georust/rstar)
- [spatialtree Docs](https://docs.rs/spatialtree)

---

## Entity Component System (ECS)

### Why ECS for CAD

- **Composition over inheritance** - Components can have any combination of traits
- **Cache-friendly** - Data stored contiguously
- **Parallelizable** - Systems can run in parallel
- **Historical precedent** - Sketchpad (1963) used ECS-like pattern!

### ECS for PCB

```rust
// Components (data)
struct Position { x: f64, y: f64 }
struct Rotation { degrees: f64 }
struct Footprint { pads: Vec<Pad>, outline: Polygon }
struct NetConnection { net_id: NetId, pin: String }
struct ComponentInfo { name: String, value: String }

// Entity = Component instance on board
// Has: Position, Rotation, Footprint, ComponentInfo, Vec<NetConnection>

// Systems (behavior)
fn placement_system(query: Query<(&mut Position, &Footprint)>) { ... }
fn drc_system(query: Query<(&Position, &Footprint)>) { ... }
fn routing_system(query: Query<&NetConnection>) { ... }
```

### Libraries

- **bevy_ecs** - Full-featured, great ergonomics
- **specs** - Mature, flexible
- **hecs** - Minimal, fast

**Real-world CAD example:**
- [arcs](https://github.com/Michael-F-Bryan/arcs) - Rust CAD library using specs

**Sources:**
- [ECS Outside Games](https://adventures.michaelfbryan.com/posts/ecs-outside-of-games/)
- [Bevy ECS](https://bevy.org/learn/quick-start/getting-started/ecs/)

---

## File Format Libraries

### Gerber (PCB Manufacturing)

**Rust crates:**
- **gerber-types** - Data types for Gerber
- **gerber_parser** - Parse Gerber files
- **gerber-viewer** - Pure Rust viewer

**Gerber X2** (modern standard):
- ASCII vector format
- Includes metadata (file attributes)
- Human-readable

### IPC-2581 (Modern Standard)

- Open XML-based format
- Single file contains everything
- No license required
- Supports HDI, rigid-flex, embedded components

**Note:** No Rust library found - may need to implement or use XML parser.

### ODB++ (Industry Standard)

- Proprietary (Siemens/Mentor)
- ZIP archive with folder structure
- Widely supported by manufacturers

### Export Priority

1. **Gerber X2** - Universal, Rust libraries exist
2. **IPC-2581** - Modern, open, XML-based
3. **ODB++** - If manufacturers require it

**Sources:**
- [gerber_parser Docs](https://docs.rs/gerber_parser/latest/gerber_parser/)
- [IPC-2581 vs ODB++](https://www.allpcb.com/allelectrohub/ipc-2581-vs-odb-choosing-the-right-data-exchange-standard-for-your-pcb-project)

---

## Serialization & Performance

### Binary Format Comparison

| Format | Size | Serialize | Deserialize | WASM |
|--------|------|-----------|-------------|------|
| bincode | Medium | Fastest | Fastest | Yes |
| MessagePack | Smallest | Medium | Medium | Yes |
| postcard | Small | Fast | Fast | Yes (embedded) |
| JSON | Largest | Medium | Medium | Yes |

**Recommendation:**
- **bincode** for internal state (fast)
- **MessagePack** for network/storage (compact)
- **JSON** for human-readable export

### Serde Performance Tips

```rust
// Zero-copy deserialization where possible
#[derive(Deserialize)]
struct BoardRef<'a> {
    name: &'a str,  // Borrows from input
    components: Vec<ComponentRef<'a>>,
}

// Use #[serde(skip)] for computed fields
#[derive(Serialize, Deserialize)]
struct Component {
    position: Point,
    #[serde(skip)]
    cached_bounds: Option<Bounds>,  // Recomputed on load
}
```

**Sources:**
- [Rust Serialization Production Ready](https://blog.logrocket.com/rust-serialization-whats-ready-for-production-today/)
- [rmp-serde Docs](https://docs.rs/rmp-serde)

---

## WebAssembly Multi-threading

### Architecture

```
┌─────────────────────────────────────────────────┐
│                  Main Thread                    │
│  - UI rendering                                 │
│  - User input                                   │
│  - Coordination                                 │
└─────────────────────┬───────────────────────────┘
                      │ SharedArrayBuffer
        ┌─────────────┼─────────────┬─────────────┐
        ▼             ▼             ▼             ▼
┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐
│  Worker 1 │   │  Worker 2 │   │  Worker 3 │   │  Worker N │
│ (routing) │   │   (DRC)   │   │  (render) │   │   (...)   │
└───────────┘   └───────────┘   └───────────┘   └───────────┘
     WASM           WASM            WASM            WASM
```

### Performance Gains

- Squoosh.app: **1.5x-3x** speedup for image compression
- Figma: **3x** improvement in load time
- Google Sheets: **2x** faster with WasmGC

### Implementation with Rust

```rust
// Using wasm-bindgen-rayon for parallel iterators
use rayon::prelude::*;

#[wasm_bindgen]
pub fn parallel_drc(board: &Board) -> Vec<Violation> {
    board.components
        .par_iter()  // Parallel iteration
        .flat_map(|c| check_component(c, board))
        .collect()
}
```

### Memory Considerations

- Use **mimalloc** over default allocator (better multi-threaded)
- **WasmFS** for file operations from multiple threads
- Avoid excessive cross-thread communication

**Sources:**
- [Scaling Multithreaded WASM](https://web.dev/articles/scaling-multithreaded-webassembly-applications)
- [WASM Threads from Rust](https://web.dev/articles/webassembly-threads)

---

## DRC Implementation

### Rule Categories

| Category | Examples |
|----------|----------|
| Clearance | Trace-trace, trace-pad, pad-pad |
| Width | Min trace width, min annular ring |
| Drill | Min hole size, aspect ratio |
| Silk | Text size, clearance from pads |
| Mask | Solder mask expansion |
| Electrical | Unconnected pins, shorted nets |

### Algorithm Approach

```rust
struct DrcEngine {
    rules: Vec<Box<dyn DrcRule>>,
    spatial_index: RTree<PcbObject>,
}

impl DrcEngine {
    fn check_all(&self, board: &Board) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Parallel check
        violations.par_extend(
            self.rules.par_iter()
                .flat_map(|rule| rule.check(board, &self.spatial_index))
        );

        violations
    }
}

trait DrcRule {
    fn check(&self, board: &Board, index: &RTree<PcbObject>) -> Vec<Violation>;
}

struct ClearanceRule {
    min_clearance: f64,
}

impl DrcRule for ClearanceRule {
    fn check(&self, board: &Board, index: &RTree<PcbObject>) -> Vec<Violation> {
        let mut violations = Vec::new();

        for obj in board.objects() {
            let nearby = index.locate_within_distance(
                obj.bounds(),
                self.min_clearance
            );

            for other in nearby {
                if obj.id != other.id && obj.distance_to(other) < self.min_clearance {
                    violations.push(Violation::Clearance {
                        objects: (obj.id, other.id),
                        actual: obj.distance_to(other),
                        required: self.min_clearance,
                    });
                }
            }
        }

        violations
    }
}
```

### Online vs Batch DRC

- **Online:** Real-time as user edits (subset of rules, incremental)
- **Batch:** Full check before export (all rules, comprehensive)

**Sources:**
- [DRC Practical Guide](https://www.elepcb.com/blog/pcb-knowledge/pcb-design-rule-check-drc-test/)
- [HyperLynx DRC](https://eda.sw.siemens.com/en-US/pcb/hyperlynx/electrical-design-rule-check/)

---

## Hot Reload & File Watching

### Development Workflow

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│  Edit .pcb   │───▶│  File Watch  │───▶│   Reparse    │
│    file      │    │   (notify)   │    │  & Render    │
└──────────────┘    └──────────────┘    └──────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │  Debounce    │
                    │   500ms      │
                    └──────────────┘
```

### Libraries

- **notify** (v6) - Cross-platform file watching
- **notify-debouncer-full** - Debounced events
- **hot-lib-reloader** - Hot reload Rust code (dev only)

### Implementation

```rust
use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

fn watch_project(path: &Path, on_change: impl Fn(&Path)) {
    let (tx, rx) = channel();

    let mut watcher = watcher(tx, Duration::from_millis(500)).unwrap();
    watcher.watch(path, RecursiveMode::Recursive).unwrap();

    loop {
        match rx.recv() {
            Ok(event) => {
                if let Some(path) = event.path {
                    if path.extension() == Some("pcb") {
                        on_change(&path);
                    }
                }
            }
            Err(e) => println!("Watch error: {:?}", e),
        }
    }
}
```

**Sources:**
- [Hot Reloading Rust](https://robert.kra.hn/posts/hot-reloading-rust/)
- [hot-lib-reloader](https://crates.io/crates/hot-lib-reloader)

---

## Autorouting Algorithms

### Algorithm Comparison

| Algorithm | Optimality | Speed | Memory | Best For |
|-----------|------------|-------|--------|----------|
| Lee (BFS) | Optimal | Slow | High | Single net, guaranteed solution |
| A* | Near-optimal | Fast | Medium | Single net, heuristic-guided |
| Dijkstra | Optimal | Medium | Medium | Weighted costs |
| Negotiation-based | Good | Fast | Low | Multiple nets simultaneously |

### Lee Algorithm

```
1. Start from source, mark as 0
2. Expand wavefront: mark adjacent cells as distance+1
3. Continue until target reached
4. Backtrace from target to source following decreasing numbers
5. Mark path as blocked for other nets
```

**Pros:** Guarantees shortest path if exists
**Cons:** O(n²) space, slow for large boards

### A* Algorithm

```
f(n) = g(n) + h(n)
- g(n) = actual cost from start
- h(n) = heuristic estimate to goal (Manhattan distance)

Priority queue ordered by f(n)
Explores most promising paths first
```

**Pros:** Much faster than Lee with good heuristic
**Cons:** Not guaranteed optimal if heuristic is inadmissible

### Multi-Net Challenge

Routing one net may block another - this is **NP-hard**.

**Strategies:**
1. **Sequential:** Route one at a time, order by criticality
2. **Rip-up and retry:** If blocked, remove previous routes and retry
3. **Negotiation-based:** All nets compete, congested areas get higher cost
4. **Genetic/simulated annealing:** Evolutionary optimization

### Implementation Approach

```rust
pub trait Router {
    fn route(&self, board: &Board, net: &Net) -> Option<Route>;
    fn route_all(&self, board: &mut Board, nets: &[Net]) -> RoutingResult;
}

struct AStarRouter {
    heuristic: Box<dyn Fn(Point, Point) -> f64>,
    cost_fn: Box<dyn Fn(&Board, Point) -> f64>,
}

impl Router for AStarRouter {
    fn route(&self, board: &Board, net: &Net) -> Option<Route> {
        let mut open = BinaryHeap::new();
        let mut came_from = HashMap::new();
        let mut g_score = HashMap::new();

        // A* implementation...
    }
}
```

**Sources:**
- [Lee Algorithm Wikipedia](https://en.wikipedia.org/wiki/Lee_algorithm)
- [Routing Wikipedia](https://en.wikipedia.org/wiki/Routing_(electronic_design_automation))
- [route1 GitHub Demo](https://github.com/iank/route1)

---

## Electrical Calculators

### Trace Width (IPC-2221)

**Formula:**
```
I = k × ΔT^0.44 × A^0.725

Where:
- I = current (Amps)
- k = 0.048 (external) or 0.024 (internal)
- ΔT = temperature rise (°C)
- A = cross-sectional area (mils²)

To find width:
A = (I / (k × ΔT^0.44))^(1/0.725)
Width = A / (thickness × 1.378)  // 1oz Cu ≈ 1.378 mils
```

**Implementation:**

```rust
pub fn trace_width_ipc2221(
    current_amps: f64,
    temp_rise_c: f64,
    copper_oz: f64,
    is_internal: bool,
) -> f64 {
    let k = if is_internal { 0.024 } else { 0.048 };
    let copper_thickness_mils = copper_oz * 1.378;

    let area_mils2 = (current_amps / (k * temp_rise_c.powf(0.44)))
        .powf(1.0 / 0.725);

    let width_mils = area_mils2 / copper_thickness_mils;
    width_mils * 0.0254  // Convert to mm
}
```

### Microstrip Impedance

**Simplified formula:**
```
Z₀ = (87 / √(εᵣ + 1.41)) × ln(5.98h / (0.8w + t))

Where:
- εᵣ = dielectric constant (FR4 ≈ 4.5)
- h = height above ground plane
- w = trace width
- t = trace thickness
```

**More accurate (Wadell's equations):**
Requires elliptical integrals - use numerical solver or lookup tables.

**Implementation:**

```rust
pub struct StackupLayer {
    pub thickness: f64,      // mm
    pub dielectric: f64,     // εᵣ
    pub copper_weight: f64,  // oz
}

pub fn microstrip_impedance(
    width: f64,
    height: f64,
    thickness: f64,
    dielectric: f64,
) -> f64 {
    let effective_er = (dielectric + 1.0) / 2.0
        + (dielectric - 1.0) / 2.0
        * (1.0 + 12.0 * height / width).powf(-0.5);

    // Hammerstad-Jensen formula
    let f = 6.0 + (2.0 * PI - 6.0) * (-((30.666 * height / width).powf(0.7528))).exp();
    let w_eff = width + thickness / PI * (1.0 + (4.0 * E.powf(1.0) / (thickness / height)).ln());

    377.0 / (f * effective_er.sqrt() + w_eff / height)
}

pub fn stripline_impedance(
    width: f64,
    height: f64,  // distance to each plane
    thickness: f64,
    dielectric: f64,
) -> f64 {
    // Simplified stripline formula
    let w_eff = width + thickness * (1.0 + (4.0 * PI * width / thickness).ln()) / (2.0 * PI);
    60.0 / dielectric.sqrt() * (4.0 * height / (0.67 * PI * (0.8 * w_eff + thickness))).ln()
}
```

### Differential Pair Impedance

```rust
pub fn differential_impedance(
    single_ended_z: f64,
    coupling_factor: f64,  // 0.0 - 1.0
) -> f64 {
    2.0 * single_ended_z * (1.0 - coupling_factor)
}
```

**Sources:**
- [IPC-2221 Calculator](https://tracewidthcalculator.com/)
- [Altium Microstrip Calculator](https://resources.altium.com/p/microstrip-impedance-calculator)
- [PCB Trace Width Guide 2025](https://www.schemalyzer.com/en/blog/pcb-design/basics/pcb-trace-width-guide)

---

## Testing Strategy

### Property-Based Testing

**Libraries:**
- **proptest** - More control, better shrinking (recommended)
- **quickcheck** - Simpler, faster generation

**Example for PCB:**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn trace_width_is_positive(
        current in 0.1f64..100.0,
        temp_rise in 5.0f64..50.0,
        copper_oz in 0.5f64..4.0,
    ) {
        let width = trace_width_ipc2221(current, temp_rise, copper_oz, false);
        prop_assert!(width > 0.0);
    }

    #[test]
    fn higher_current_needs_wider_trace(
        current1 in 0.1f64..50.0,
        current2 in 50.0f64..100.0,
        temp_rise in 10.0f64..30.0,
    ) {
        let w1 = trace_width_ipc2221(current1, temp_rise, 1.0, false);
        let w2 = trace_width_ipc2221(current2, temp_rise, 1.0, false);
        prop_assert!(w2 > w1);
    }

    #[test]
    fn drc_roundtrip(board in arb_board()) {
        let violations = drc_check(&board);
        // After fixing all violations, DRC should pass
        let fixed = fix_violations(&board, &violations);
        let new_violations = drc_check(&fixed);
        prop_assert!(new_violations.is_empty());
    }
}
```

### Snapshot Testing

```rust
use insta::assert_snapshot;

#[test]
fn gerber_output_stable() {
    let board = create_test_board();
    let gerber = generate_gerber(&board);
    assert_snapshot!(gerber);
}

#[test]
fn parsed_board_matches() {
    let source = include_str!("fixtures/simple.pcb");
    let board = parse_pcb(source).unwrap();
    assert_snapshot!(format!("{:#?}", board));
}
```

### Integration Tests

```rust
#[test]
fn full_design_flow() {
    // Parse → Validate → Route → DRC → Export
    let source = include_str!("fixtures/test_design.pcb");
    let mut board = parse_pcb(source).unwrap();

    assert!(validate_netlist(&board).is_ok());

    let routing_result = autoroute(&mut board);
    assert!(routing_result.success);

    let drc_result = drc_check(&board);
    assert!(drc_result.is_empty(), "DRC violations: {:?}", drc_result);

    let gerber = generate_gerber(&board);
    assert!(!gerber.is_empty());
}
```

**Sources:**
- [Proptest Crate](https://crates.io/crates/proptest)
- [Property-Based Testing in Rust](https://blog.logrocket.com/property-based-testing-in-rust-with-proptest/)

---

## Math Libraries

### Recommendation: **nalgebra** + **glam**

| Library | Best For | Status |
|---------|----------|--------|
| **nalgebra** | General linear algebra, matrices, transforms | Active, mature |
| **glam** | Fast 3D math, SIMD-optimized | Active, game-focused |
| cgmath | Legacy | Unmaintained since 2021 |

**Usage pattern:**

```rust
// nalgebra for general math
use nalgebra::{Matrix4, Point2, Point3, Vector2, Vector3};

// glam for hot paths (rendering)
use glam::{Vec2, Vec3, Mat4};

// Convert between them at boundaries
impl From<nalgebra::Point2<f64>> for glam::DVec2 {
    fn from(p: nalgebra::Point2<f64>) -> Self {
        glam::DVec2::new(p.x, p.y)
    }
}
```

**Sources:**
- [mathbench-rs](https://github.com/bitshifter/mathbench-rs)
- [nalgebra](https://nalgebra.org/)

---

## Error Handling

### Pattern: thiserror (library) + anyhow (app)

**Library errors (thiserror):**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unexpected token at line {line}: expected {expected}, found {found}")]
    UnexpectedToken {
        line: usize,
        expected: String,
        found: String,
    },

    #[error("unknown component: {0}")]
    UnknownComponent(String),

    #[error("invalid dimension: {0}")]
    InvalidDimension(#[from] std::num::ParseFloatError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum DrcError {
    #[error("clearance violation between {obj1} and {obj2}: {actual}mm < {required}mm")]
    Clearance {
        obj1: ObjectId,
        obj2: ObjectId,
        actual: f64,
        required: f64,
    },

    #[error("trace too narrow: {actual}mm < {required}mm for {current}A")]
    TraceWidth {
        actual: f64,
        required: f64,
        current: f64,
    },
}
```

**Application errors (anyhow):**

```rust
use anyhow::{Context, Result};

fn load_project(path: &Path) -> Result<Project> {
    let content = std::fs::read_to_string(path)
        .context("failed to read project file")?;

    let board = parse_pcb(&content)
        .context("failed to parse PCB")?;

    let footprints = load_footprints(&board)
        .context("failed to load footprints")?;

    Ok(Project { board, footprints })
}
```

**Sources:**
- [Rust Error Handling Guide 2025](https://markaicode.com/rust-error-handling-2025-guide/)
- [thiserror vs anyhow](https://momori.dev/posts/rust-error-handling-thiserror-anyhow/)

---

## Canvas Rendering Strategy

### Recommended Approach

```
┌─────────────────────────────────────────────────┐
│               JavaScript Side                   │
│  - Canvas2D context                             │
│  - Event handling                               │
│  - requestAnimationFrame                        │
└─────────────────────┬───────────────────────────┘
                      │ Call WASM for:
                      │ - Hit testing
                      │ - Geometry calculations
                      │ - State management
                      ▼
┌─────────────────────────────────────────────────┐
│               Rust/WASM Side                    │
│  - Board state (components, nets, routes)       │
│  - Spatial indexing (R-tree)                    │
│  - DRC engine                                   │
│  - Export functions                             │
│  Returns: Draw commands / geometry              │
└─────────────────────────────────────────────────┘
```

**Why this split:**
- Canvas2D calls from JS are already fast (<1ms per frame)
- WASM excels at state management and computation
- Avoid overhead of passing large pixel buffers

**Implementation:**

```rust
// WASM side - returns draw commands
#[wasm_bindgen]
pub fn get_render_commands(board: &Board, viewport: Viewport) -> JsValue {
    let mut commands = Vec::new();

    for component in board.visible_components(&viewport) {
        commands.push(DrawCommand::Component {
            outline: component.outline.clone(),
            pads: component.pads.clone(),
            position: component.position,
            rotation: component.rotation,
        });
    }

    for trace in board.visible_traces(&viewport) {
        commands.push(DrawCommand::Trace {
            points: trace.points.clone(),
            width: trace.width,
            layer: trace.layer,
        });
    }

    serde_wasm_bindgen::to_value(&commands).unwrap()
}
```

```javascript
// JS side - executes draw commands
function render(ctx, commands) {
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    for (const cmd of commands) {
        switch (cmd.type) {
            case 'Component':
                drawComponent(ctx, cmd);
                break;
            case 'Trace':
                drawTrace(ctx, cmd);
                break;
        }
    }
}
```

**Sources:**
- [rust-wasm Book](https://rustwasm.github.io/docs/book/)
- [Reactive Canvas with Rust/WASM](https://dev.to/deciduously/reactive-canvas-with-rust-webassembly-and-web-sys-2hg2)

---

## Complete Rust Crate Dependencies

```toml
[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Math
nalgebra = "0.33"
glam = "0.29"

# Spatial indexing
rstar = "0.12"

# Parsing
tree-sitter = "0.25"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Serialization
bincode = "1.3"
rmp-serde = "1.3"  # MessagePack

# File watching
notify = "7.0"
notify-debouncer-full = "0.4"

# LSP
tower-lsp = "0.20"
lsp-types = "0.97"

# WASM
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["CanvasRenderingContext2d", "HtmlCanvasElement"] }
serde-wasm-bindgen = "0.6"

# Testing
proptest = "1.5"
insta = "1.40"

# Async
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
criterion = "0.5"  # Benchmarking

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
# Desktop-only
tauri = "2.0"

[lib]
crate-type = ["cdylib", "rlib"]
```

---

*Last updated: during initial brainstorm session*
