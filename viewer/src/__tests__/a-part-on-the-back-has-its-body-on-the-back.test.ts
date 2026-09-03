import { describe, it, expect } from 'vitest';
import { bodyPlacement } from '../renderer3d';

/**
 * A part on the back of the board has its body on the back of the board.
 *
 * `npx vitest run src/__tests__/a-part-on-the-back-has-its-body-on-the-back.test.ts`
 *
 * The 3D view put every component body in the top group, whatever side the
 * part was on, under a comment that said so: "on top layer group for now, all
 * top-side". Its pads and its silkscreen did respect the side, so a part on
 * the back of a double-sided board had its body floating above the board while
 * its own copper and legend were drawn underneath it.
 *
 * The second fault was in the same line. The substrate slab is centred on
 * `z = 0`, so its faces are at plus and minus half its thickness - and the
 * body was placed at a full `BOARD_THICKNESS_MM`, which is half a board too
 * high. Every part on a 1.6mm board stood on 0.8mm of air.
 *
 * What is checked here is the decision, not the three.js: which side a body
 * goes to and what height it sits at.
 */

/** The faces of a 1.6mm board, which is what the default thickness is. */
const TOP = 0.8;
const BOT = -0.8;

/** A 1mm-tall surface-mount body. */
const HEIGHT = 1.0;

describe('bodyPlacement', () => {
  it('stands a top-side body on the top face', () => {
    const place = bodyPlacement('top', HEIGHT, TOP, BOT);

    expect(place.onBottom).toBe(false);
    expect(place.centreZ).toBeCloseTo(TOP + HEIGHT / 2, 10);
    // The half that matters: its underside touches the board rather than
    // hovering half a board above it.
    expect(place.centreZ - HEIGHT / 2).toBeCloseTo(TOP, 10);
  });

  it('hangs a bottom-side body under the bottom face', () => {
    const place = bodyPlacement('bottom', HEIGHT, TOP, BOT);

    expect(place.onBottom).toBe(true);
    expect(place.centreZ).toBeCloseTo(BOT - HEIGHT / 2, 10);
    expect(place.centreZ + HEIGHT / 2).toBeCloseTo(BOT, 10);
    expect(place.centreZ).toBeLessThan(BOT);
  });

  it('puts each label clear of its own body, on its own side', () => {
    const top = bodyPlacement('top', HEIGHT, TOP, BOT);
    const bottom = bodyPlacement('bottom', HEIGHT, TOP, BOT);

    expect(top.labelZ).toBeGreaterThan(top.centreZ + HEIGHT / 2);
    expect(bottom.labelZ).toBeLessThan(bottom.centreZ - HEIGHT / 2);
  });

  it('treats a part that states no side as a top-side part', () => {
    // The control. Most boards state nothing, and reading silence as `bottom`
    // would move every part on them.
    expect(bodyPlacement(undefined, HEIGHT, TOP, BOT)).toEqual(
      bodyPlacement('top', HEIGHT, TOP, BOT),
    );
  });

  it('follows the board it is given rather than a remembered thickness', () => {
    // A 0.6mm board, so a placement that reached for a constant instead of the
    // face it was handed lands somewhere else entirely.
    const thin = bodyPlacement('top', HEIGHT, 0.3, -0.3);
    expect(thin.centreZ).toBeCloseTo(0.3 + HEIGHT / 2, 10);
  });
});
