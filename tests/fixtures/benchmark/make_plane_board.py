#!/usr/bin/env python3
"""Emit `plane_board.kicad_pcb`, the first benchmark with a ground plane.

Every routing number this project publishes was measured on a board without
one. That is not a small gap: a plane changes what the router is solving.
Without it, GND is a net like any other and every ground pin has to be reached
by a trace - on `shift_driver` that is a quarter of all the routing. With it,
GND is copper that already exists, so those pins are connected the moment the
pour is filled, and the plane becomes an obstacle the signal nets have to
respect instead of a net competing with them.

The circuit: a sensor-hub shape - an SOIC-14 logic part, an SOT-23-5 regulator,
a row of decoupling capacitors, pull-ups, and two headers - on two layers, with
the whole bottom layer poured to GND. Every part has a ground pin, so the plane
carries real traffic rather than sitting decoratively in a corner.

The pour is a rectangle inset 1mm from the board edge. Rectangular because the
importer carries a rectangular outline exactly and refuses anything else by
name; 1mm in because copper to the board edge is its own violation and this
fixture is meant to measure routing, not to fail edge clearance 40 times.

Nothing here knows what a pad zone or a via price is. The circuit is declared
as parts and nets and handed to `kicad_emit`.

Run:  python3 tests/fixtures/benchmark/make_plane_board.py
"""

from pathlib import Path

from kicad_emit import chip, emit, header, pour, report

BOARD_W, BOARD_H = 50.0, 38.0
ORIGIN_X, ORIGIN_Y = 100.0, 60.0

# The pour, inset from the edge on the bottom layer.
POUR_INSET = 1.0


def soic14(x, y, ref, value, pin_nets):
    """SOIC-14: 1.27mm pitch, 7 pins a side, pads 1.55 x 0.6mm on a 5.4mm span."""
    pitch, span = 1.27, 5.4
    pads = []
    offset = pitch * 3  # (7 - 1) / 2
    for i in range(7):  # left side, top to bottom
        pads.append(
            (str(1 + i), "smd", "roundrect", -span / 2, -offset + i * pitch,
             1.55, 0.6, None, pin_nets.get(1 + i))
        )
    for i in range(7):  # right side, bottom to top
        pads.append(
            (str(8 + i), "smd", "roundrect", span / 2, offset - i * pitch,
             1.55, 0.6, None, pin_nets.get(8 + i))
        )
    return ("Package_SO:SOIC-14_3.9x8.7mm_P1.27mm", ref, value, x, y, pads)


def sot23_5(x, y, ref, value, pin_nets):
    """SOT-23-5: three pads one side, two the other, 0.95mm pitch."""
    pads = []
    for i in range(3):
        pads.append(
            (str(1 + i), "smd", "roundrect", -0.9375, -0.95 + i * 0.95,
             1.0, 0.6, None, pin_nets.get(1 + i))
        )
    for i, pin in enumerate((5, 4)):
        pads.append(
            (str(pin), "smd", "roundrect", 0.9375, -0.475 + i * 0.95,
             1.0, 0.6, None, pin_nets.get(pin))
        )
    return ("Package_TO_SOT_SMD:SOT-23-5", ref, value, x, y, pads)


def cap_0603(x, y, ref, net1, net2, rotate=False):
    """0603: pads 0.9 x 0.95mm on a 1.6mm span."""
    return chip("Capacitor_SMD:C_0603_1608Metric", ref, "100nF", x, y,
                net1, net2, 0.9, 0.95, 1.6, rotate=rotate)


def res_0603(x, y, ref, value, net1, net2, rotate=False):
    return chip("Resistor_SMD:R_0603_1608Metric", ref, value, x, y,
                net1, net2, 0.9, 0.95, 1.6, rotate=rotate)


def build():
    nets = ["", "GND", "VIN", "VCC", "SCL", "SDA", "INT", "RST"]
    for i in range(8):
        nets.append(f"IO{i}")

    parts = []

    # The logic part, centred. Pin 7 is ground and pin 14 is supply, the
    # arrangement most 14-pin logic uses.
    u1_pins = {7: "GND", 14: "VCC", 1: "SCL", 2: "SDA", 3: "INT", 4: "RST"}
    for i, io in enumerate(range(5, 7)):
        u1_pins[io] = f"IO{i}"
    for i, io in enumerate(range(8, 14)):
        u1_pins[io] = f"IO{i + 2}"
    parts.append(soic14(20.0, 19.0, "U1", "SENSOR-HUB", u1_pins))

    # Regulator: VIN in, VCC out, ground on pin 2.
    parts.append(sot23_5(8.0, 30.0, "U2", "LDO",
                         {1: "VIN", 2: "GND", 3: "VIN", 4: "GND", 5: "VCC"}))

    # Decoupling, one per supply pin plus the regulator's pair.
    # 4mm apart, not 3. An 0603 courtyard is about 3mm wide once the IPC
    # excess is added either side, so two of them 3mm apart touch at exactly
    # 0.00mm - which `cypcb check` reported on this fixture from the day it
    # was generated. A fixture that claims to be a board somebody could make
    # has to be one.
    parts.append(cap_0603(12.5, 30.0, "C1", "VIN", "GND"))
    parts.append(cap_0603(16.5, 30.0, "C2", "VCC", "GND"))
    parts.append(cap_0603(20.0, 12.0, "C3", "VCC", "GND"))
    parts.append(cap_0603(24.0, 26.0, "C4", "VCC", "GND", rotate=True))

    # I2C pull-ups and the reset pull-up.
    parts.append(res_0603(31.0, 22.0, "R1", "4k7", "VCC", "SCL"))
    parts.append(res_0603(31.0, 19.0, "R2", "4k7", "VCC", "SDA"))
    parts.append(res_0603(31.0, 16.0, "R3", "10k", "VCC", "RST"))

    # Power in on the left edge, signals out on the right.
    parts.append(header("Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical",
                        "J1", "PWR", 3.0, 8.0, ["VIN", "GND", "VCC", "GND"]))
    parts.append(header("Connector_PinHeader_2.54mm:PinHeader_1x08_P2.54mm_Vertical",
                        "J2", "IO", 47.0, 8.0,
                        ["SCL", "SDA", "INT", "RST", "IO0", "IO1", "GND", "VCC"]))
    parts.append(header("Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Horizontal",
                        "J3", "AUX", 14.0, 35.0,
                        ["IO2", "IO3", "IO4", "IO5", "IO6", "GND"],
                        along_x=True))

    pours = [
        pour("GND", "B.Cu", POUR_INSET, POUR_INSET,
             BOARD_W - POUR_INSET, BOARD_H - POUR_INSET),
    ]

    return nets, parts, pours


if __name__ == "__main__":
    nets, parts, pours = build()
    target = Path(__file__).with_name("plane_board.kicad_pcb")
    target.write_text(
        emit(BOARD_W, BOARD_H, ORIGIN_X, ORIGIN_Y, nets, parts, pours)
    )
    report(target, parts, nets, BOARD_W, BOARD_H)
    ground_pins = sum(
        1 for _, _, _, _, _, pads in parts for pad in pads if pad[8] == "GND"
    )
    print(f"  ground pins on the plane: {ground_pins}")
