#!/usr/bin/env python3
"""Emit `shift_driver.kicad_pcb`, a fourth benchmark board.

Every routing conclusion in `docs/routing.md` rests on three fixtures, and the
next step there - choosing the pad opening from the board rather than fixing it
- cannot be told apart from a curve fit until there is a board nobody fitted
on. This is that board.

It is generated rather than hand-written so its provenance is checkable: the
circuit is declared below as parts and nets, and nothing in this file knows
what a pad zone is or which routing setting wins. The layout is a plain grid
placement, the same one anybody would reach for.

The circuit: three 74HC595 shift registers in a chain, each driving eight LEDs
through their own resistors, with a bypass capacitor per register and a 6-pin
header for power and the serial interface. Through-hole DIP-16 beside 0805
chips, which is a mix none of the other three fixtures has - they are
SMD-dominant.

Run:  python3 tests/fixtures/benchmark/make_shift_driver.py
"""

from pathlib import Path

BOARD_W, BOARD_H = 68.0, 48.0
ORIGIN_X, ORIGIN_Y = 100.0, 60.0

REGISTERS = 3
LEDS_PER_REGISTER = 8


def dip16(x, y, ref, pins):
    """A through-hole DIP-16: two rows of eight on a 2.54mm pitch, 7.62 apart."""
    pads = []
    for i in range(8):
        pads.append((str(i + 1), "thru_hole", "rect" if i == 0 else "oval",
                     0.0, i * 2.54, 1.6, 1.6, 0.8, pins.get(str(i + 1))))
    for i in range(8):
        pads.append((str(16 - i), "thru_hole", "oval",
                     7.62, i * 2.54, 1.6, 1.6, 0.8, pins.get(str(16 - i))))
    return ("Package_DIP:DIP-16_W7.62mm", ref, "74HC595", x, y, pads)


def chip_0805(x, y, ref, value, net1, net2, rotate=0):
    """A two-terminal 0805, pads 1.0 x 1.45mm at +/-0.95mm."""
    if rotate:
        offsets = [(0.0, -0.95), (0.0, 0.95)]
        size = (1.45, 1.0)
    else:
        offsets = [(-0.95, 0.0), (0.95, 0.0)]
        size = (1.0, 1.45)
    pads = [
        ("1", "smd", "roundrect", offsets[0][0], offsets[0][1], size[0], size[1], None, net1),
        ("2", "smd", "roundrect", offsets[1][0], offsets[1][1], size[0], size[1], None, net2),
    ]
    return ("Resistor_SMD:R_0805_2012Metric", ref, value, x, y, pads)


def header_1x6(x, y, ref, nets):
    pads = [
        (str(i + 1), "thru_hole", "rect" if i == 0 else "oval",
         0.0, i * 2.54, 1.7, 1.7, 1.0, nets[i])
        for i in range(6)
    ]
    return ("Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Vertical",
            ref, "Ctrl", x, y, pads)


def build():
    """The board, as (nets, footprints)."""
    nets = ["", "VCC", "GND", "SER", "SRCLK", "RCLK"]
    parts = []

    # Control header down the left edge.
    parts.append(header_1x6(3.0, 6.0, "J1", ["VCC", "GND", "SER", "SRCLK", "RCLK", "GND"]))

    for reg in range(REGISTERS):
        ref = f"U{reg + 1}"
        # Each register's serial input is the previous one's overflow output.
        serial_in = "SER" if reg == 0 else f"CHAIN{reg}"
        serial_out = f"CHAIN{reg + 1}"
        if serial_out not in nets and reg + 1 < REGISTERS:
            nets.append(serial_out)

        outputs = {}
        for bit in range(LEDS_PER_REGISTER):
            index = reg * LEDS_PER_REGISTER + bit
            drive = f"Q{index}"
            anode = f"A{index}"
            nets += [drive, anode]
            outputs[bit] = drive

        # 74HC595 pinout: QB..QH on 1-7, GND 8, QH' 9, SRCLR 10, SRCLK 11,
        # RCLK 12, OE 13, SER 14, QA 15, VCC 16.
        pins = {
            "1": outputs[1], "2": outputs[2], "3": outputs[3], "4": outputs[4],
            "5": outputs[5], "6": outputs[6], "7": outputs[7], "8": "GND",
            "9": serial_out if reg + 1 < REGISTERS else "GND",
            "10": "VCC", "11": "SRCLK", "12": "RCLK", "13": "GND",
            "14": serial_in, "15": outputs[0], "16": "VCC",
        }
        parts.append(dip16(12.0 + reg * 18.0, 4.0, ref, pins))

        # Bypass capacitor beside every register.
        parts.append(chip_0805(12.0 + reg * 18.0 + 3.8, -3.4, f"C{reg + 1}", "100nF",
                               "VCC", "GND"))

        # Eight resistor/LED pairs per register, in two columns below it.
        for bit in range(LEDS_PER_REGISTER):
            index = reg * LEDS_PER_REGISTER + bit
            column = bit % 2
            row = bit // 2
            x = 11.0 + reg * 18.0 + column * 9.5
            y = 25.0 + row * 4.5
            parts.append(chip_0805(x, y, f"R{index + 1}", "330R",
                                   f"Q{index}", f"A{index}"))
            parts.append(chip_0805(x + 4.2, y, f"D{index + 1}", "RED",
                                   f"A{index}", "GND"))

    return nets, parts


LAYER_BLOCK = """  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (32 "B.Adhes" user "B.Adhesive")
    (33 "F.Adhes" user "F.Adhesive")
    (34 "B.Paste" user)
    (35 "F.Paste" user)
    (36 "B.SilkS" user "B.Silkscreen")
    (37 "F.SilkS" user "F.Silkscreen")
    (38 "B.Mask" user)
    (39 "F.Mask" user)
    (40 "Dwgs.User" user "User.Drawings")
    (41 "Cmts.User" user "User.Comments")
    (44 "Edge.Cuts" user)
  )"""


def emit(nets, parts):
    index = {name: i for i, name in enumerate(nets)}
    out = ['(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")', ""]
    out += ["  (general", "    (thickness 1.6)", "    (legacy_teardrops no)", "  )", ""]
    out += ['  (paper "A4")', "", LAYER_BLOCK, ""]
    out += ["  (setup", "    (pad_to_mask_clearance 0)",
            "    (allow_soldermask_bridges_in_footprints no)", "  )", ""]
    for name in nets:
        out.append(f'  (net {index[name]} "{name}")')
    out.append("")
    out.append(
        f"  (gr_rect (start {ORIGIN_X} {ORIGIN_Y}) "
        f"(end {ORIGIN_X + BOARD_W} {ORIGIN_Y + BOARD_H}) "
        '(layer "Edge.Cuts") (width 0.05)'
    )
    out += ["    (stroke (width 0.05) (type default))", "  )", ""]

    for library, ref, value, x, y, pads in parts:
        at_x = ORIGIN_X + x
        at_y = ORIGIN_Y + y
        out.append(f'  (footprint "{library}"')
        out.append('    (layer "F.Cu")')
        out.append(f"    (at {at_x:.3f} {at_y:.3f})")
        out.append(f'    (property "Reference" "{ref}")')
        out.append(f'    (property "Value" "{value}")')
        out.append(f'    (property "Footprint" "{library}")')
        for number, kind, shape, px, py, sx, sy, drill, net in pads:
            drill_part = f" (drill {drill})" if drill else ""
            layers = '"*.Cu" "*.Mask"' if kind == "thru_hole" else '"F.Cu" "F.Paste" "F.Mask"'
            net_part = f' (net {index[net]} "{net}")' if net else ""
            out.append(
                f'    (pad "{number}" {kind} {shape} (at {px} {py}) '
                f"(size {sx} {sy}){drill_part} (layers {layers}){net_part})"
            )
        out.append("  )")
        out.append("")

    out.append(")")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    nets, parts = build()
    target = Path(__file__).with_name("shift_driver.kicad_pcb")
    target.write_text(emit(nets, parts))
    pads = sum(len(p[5]) for p in parts)
    print(f"{target.name}: {len(parts)} parts, {pads} pads, {len(nets) - 1} nets, "
          f"{BOARD_W}x{BOARD_H}mm")
