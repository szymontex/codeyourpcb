#!/usr/bin/env python3
"""Emit `qfp_fanout.kicad_pcb`, a fifth benchmark board.

The four boards before it left one regime untouched: a **fine-pitch part on two
layers**. `stm32_breakout` has a TQFP at 0.8mm pitch, `multi_ic` has an LQFP-100
but on four layers with room to escape into. Escaping 64 pins at 0.5mm pitch
with nothing but a top and a bottom is the case a two-layer router is hardest
on, and no conclusion in `docs/routing.md` has ever been checked against it.

The circuit: one LQFP-64 with a decoupling capacitor per supply pin, a crystal
and its two load capacitors, a reset network, and headers taking the GPIO out
to the edges. Every net that leaves the part has somewhere to go, so the router
cannot dodge the fanout by leaving pins unconnected.

Nothing here knows what a pad zone is or which routing setting wins - the
circuit is declared as parts and nets and handed to `kicad_emit`.

Run:  python3 tests/fixtures/benchmark/make_qfp_fanout.py
"""

from pathlib import Path

from kicad_emit import chip, emit, header, report

BOARD_W, BOARD_H = 46.0, 46.0
ORIGIN_X, ORIGIN_Y = 100.0, 60.0

# LQFP-64: 10x10mm body, 0.5mm pitch, 16 pins a side, pads 1.5 x 0.3mm on a
# 11.4mm span between opposite rows.
PITCH = 0.5
PINS_PER_SIDE = 16
ROW_SPAN = 11.4
PAD_LONG, PAD_SHORT = 1.5, 0.3

CENTRE_X, CENTRE_Y = BOARD_W / 2, BOARD_H / 2 - 2.0


def lqfp64(x, y, ref, pin_nets):
    """Pins counter-clockwise from the top of the left side, as KiCad numbers them."""
    pads = []
    half = ROW_SPAN / 2
    offset = PITCH * (PINS_PER_SIDE - 1) / 2

    def add(number, px, py, sx, sy):
        pads.append((str(number), "smd", "roundrect", px, py, sx, sy, None,
                     pin_nets.get(number)))

    for i in range(PINS_PER_SIDE):  # left, top to bottom
        add(1 + i, -half, -offset + i * PITCH, PAD_LONG, PAD_SHORT)
    for i in range(PINS_PER_SIDE):  # bottom, left to right
        add(17 + i, -offset + i * PITCH, half, PAD_SHORT, PAD_LONG)
    for i in range(PINS_PER_SIDE):  # right, bottom to top
        add(33 + i, half, offset - i * PITCH, PAD_LONG, PAD_SHORT)
    for i in range(PINS_PER_SIDE):  # top, right to left
        add(49 + i, offset - i * PITCH, -half, PAD_SHORT, PAD_LONG)

    return ("Package_QFP:LQFP-64_10x10mm_P0.5mm", ref, "MCU", x, y, pads)


def cap_0402(x, y, ref, net1, net2, rotate=False):
    """0402: pads 0.6 x 0.5mm on a 1.0mm span."""
    return chip("Capacitor_SMD:C_0402_1005Metric", ref, "100nF", x, y,
                net1, net2, 0.6, 0.5, 1.0, rotate=rotate)


def build():
    nets = ["", "VCC", "GND", "NRST", "XTAL_IN", "XTAL_OUT", "BOOT"]

    # Every pin that is not power, ground or one of the named signals goes to a
    # header, so the fanout has to leave the part on both layers.
    gpio_pins = []
    pin_nets = {}
    supply_pins = [1, 16, 17, 32, 33, 48, 49, 64]
    ground_pins = [2, 15, 18, 31, 34, 47, 50, 63]
    named = {5: "NRST", 6: "XTAL_IN", 7: "XTAL_OUT", 8: "BOOT"}

    for pin in range(1, 65):
        if pin in supply_pins:
            pin_nets[pin] = "VCC"
        elif pin in ground_pins:
            pin_nets[pin] = "GND"
        elif pin in named:
            pin_nets[pin] = named[pin]
        else:
            net = f"IO{len(gpio_pins)}"
            nets.append(net)
            pin_nets[pin] = net
            gpio_pins.append(net)

    parts = [lqfp64(CENTRE_X, CENTRE_Y, "U1", pin_nets)]

    # A decoupling capacitor per supply pin, ringed outside the part.
    ring = 9.5
    corners = [(-ring, -ring), (ring, -ring), (ring, ring), (-ring, ring),
               (-ring, 0.0), (ring, 0.0), (0.0, -ring), (0.0, ring)]
    for i, (dx, dy) in enumerate(corners):
        parts.append(cap_0402(CENTRE_X + dx, CENTRE_Y + dy, f"C{i + 1}",
                              "VCC", "GND", rotate=abs(dx) > abs(dy)))

    # Crystal and its load capacitors, below the part.
    parts.append(chip("Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm", "Y1", "8MHz",
                      CENTRE_X - 6.0, CENTRE_Y + 13.0,
                      "XTAL_IN", "XTAL_OUT", 1.4, 1.2, 2.2))
    parts.append(cap_0402(CENTRE_X - 8.0, CENTRE_Y + 15.4, "C9", "XTAL_IN", "GND"))
    parts.append(cap_0402(CENTRE_X - 4.0, CENTRE_Y + 15.4, "C10", "XTAL_OUT", "GND"))

    # Reset network.
    parts.append(chip("Resistor_SMD:R_0402_1005Metric", "R1", "10k",
                      CENTRE_X + 4.0, CENTRE_Y + 13.0, "VCC", "NRST", 0.6, 0.5, 1.0))
    parts.append(cap_0402(CENTRE_X + 4.0, CENTRE_Y + 15.4, "C11", "NRST", "GND"))
    parts.append(chip("Resistor_SMD:R_0402_1005Metric", "R2", "10k",
                      CENTRE_X + 8.0, CENTRE_Y + 13.0, "BOOT", "GND", 0.6, 0.5, 1.0))

    # Headers around the edges, taking the GPIO off the board.
    #
    # Four of twelve, one per edge. The first version of this fixture used two
    # of twenty-one down the sides: 50.8mm of pins on a 46mm board, so three
    # pads per header sat past the board outline. That is not a board anybody
    # could make, and the router was being measured on it - `J1 <-> trace`
    # violations at y = 45.5 to 46.1 on a board 46mm tall were the giveaway.
    #
    # A 46mm edge holds fifteen pins at 2.54mm once margins are taken, so two
    # sides cannot hold forty-eight. All four can.
    per_header = 12
    groups = [gpio_pins[i:i + per_header - 1] for i in range(0, len(gpio_pins), per_header - 1)]
    assert len(groups) == 4, f"expected four headers, got {len(groups)}"
    spare = ["VCC", "GND", "VCC", "GND"]
    run = (per_header - 1) * 2.54
    edge = 3.0
    centred = (BOARD_W - run) / 2
    placements = [
        (edge, centred, False),
        (BOARD_W - edge, centred, False),
        (centred, edge, True),
        (centred, BOARD_H - edge, True),
    ]
    for index, (pins, (x, y, along_x)) in enumerate(zip(groups, placements)):
        # The horizontal pair gets a library name of its own. The importer
        # keys its footprint library by library name and registers the first
        # part it sees under that name - `if !library.contains(&library_key)` -
        # so two headers sharing a name and differing in geometry collapse into
        # whichever was read first. Both would have come out running in y, and
        # the two along the top and bottom edges would have run off the board
        # in the model while the file said otherwise.
        library = (
            "Connector_PinHeader_2.54mm:PinHeader_1x12_P2.54mm_Horizontal"
            if along_x
            else "Connector_PinHeader_2.54mm:PinHeader_1x12_P2.54mm_Vertical"
        )
        parts.append(header(library, f"J{index + 1}", "GPIO", x, y,
                            pins + [spare[index]], along_x=along_x))

    return nets, parts


if __name__ == "__main__":
    nets, parts = build()
    target = Path(__file__).with_name("qfp_fanout.kicad_pcb")
    target.write_text(emit(BOARD_W, BOARD_H, ORIGIN_X, ORIGIN_Y, nets, parts))
    report(target, parts, nets, BOARD_W, BOARD_H)
