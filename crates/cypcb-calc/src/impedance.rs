//! IPC-2141 impedance for a microstrip and a symmetric stripline.
//!
//! # Where these come from, and how far to trust them
//!
//! Two closed forms, both IPC-2141's:
//!
//! ```text
//! microstrip   Z0 = 87 / sqrt(Er + 1.41) * ln( 5.98 H / (0.8 W + T) )
//! stripline    Z0 = 60 / sqrt(Er)        * ln( 4 B / (0.67 pi (0.8 W + T)) )
//! ```
//!
//! `Er` is the dielectric constant, `W` the trace width, `T` the copper
//! thickness, `H` the height of the trace above its reference plane, and `B`
//! the separation between the two planes a stripline runs between.
//!
//! **This project has not checked either against a third party.** IPC-2141 is
//! not a public document; the equations above are the form the standard is
//! quoted in across published calculators, and the constants were read off
//! those rather than off the standard. The accuracy usually quoted alongside
//! them is about 5-7% for the microstrip and about 1% for the stripline. That
//! is the same position `RulesPreset::provenance` takes about the IPC rule
//! tables, and the same rule applies to a reader: check a controlled-impedance
//! stack against your fabricator's own calculator before committing an order.
//!
//! Published validity ranges, stated here rather than enforced, because the
//! symbols they are written in do not map cleanly onto these parameters:
//! `W/(H-T) < 0.35` and `T/H < 0.25` for the stripline. What **is** enforced
//! is `1 < Er < 15`, which both forms share, and that every dimension is
//! positive and the logarithm's argument is greater than one - below that the
//! formula returns zero or a negative impedance, which is not a small error
//! but a meaningless one.
//!
//! # Units
//!
//! Dimensions are [`Nm`]. `Er` is in thousandths, matching
//! `StackupLayer::dk_x1000`, and the result is in hundredths of an ohm,
//! matching `DesignConstraints::default_impedance_ohms_x100`. Neither is a
//! float, because both feed structures that are `Eq` and `Hash`.

use cypcb_core::Nm;

/// The narrowest and widest dielectric constant either form is quoted for.
const DK_X1000_MIN: u32 = 1_000;
const DK_X1000_MAX: u32 = 15_000;

/// `0.8 W + T`, the effective conductor width both forms use.
///
/// Returns `None` when either dimension is zero or negative, which is a board
/// nobody can build rather than a number worth reporting.
fn effective_width_mm(width: Nm, copper: Nm) -> Option<f64> {
    if width.raw() <= 0 || copper.raw() <= 0 {
        return None;
    }
    Some(0.8 * width.to_mm() + copper.to_mm())
}

fn dk(dk_x1000: u32) -> Option<f64> {
    if dk_x1000 <= DK_X1000_MIN || dk_x1000 >= DK_X1000_MAX {
        return None;
    }
    Some(f64::from(dk_x1000) / 1_000.0)
}

/// Round an impedance in ohms to hundredths, refusing what the formula cannot
/// answer.
fn ohms_x100(z: f64) -> Option<u32> {
    if !z.is_finite() || z <= 0.0 {
        return None;
    }
    Some((z * 100.0).round() as u32)
}

/// A trace on an outer layer over one reference plane.
///
/// `height` is the dielectric between the trace and the plane under it - on a
/// four-layer board that is the top prepreg, not the whole board.
///
/// Returns `None` when the inputs are outside what the form answers for:
/// see the module note.
pub fn microstrip_ohms_x100(width: Nm, height: Nm, copper: Nm, dk_x1000: u32) -> Option<u32> {
    let effective = effective_width_mm(width, copper)?;
    let er = dk(dk_x1000)?;
    if height.raw() <= 0 {
        return None;
    }
    let ratio = 5.98 * height.to_mm() / effective;
    if ratio <= 1.0 {
        return None;
    }
    ohms_x100(87.0 / (er + 1.41).sqrt() * ratio.ln())
}

/// A trace on an inner layer, centred between two reference planes.
///
/// `plate_separation` is the distance between the two planes, not the distance
/// from the trace to one of them.
pub fn stripline_ohms_x100(
    width: Nm,
    plate_separation: Nm,
    copper: Nm,
    dk_x1000: u32,
) -> Option<u32> {
    let effective = effective_width_mm(width, copper)?;
    let er = dk(dk_x1000)?;
    if plate_separation.raw() <= 0 {
        return None;
    }
    let ratio = 4.0 * plate_separation.to_mm() / (0.67 * std::f64::consts::PI * effective);
    if ratio <= 1.0 {
        return None;
    }
    ohms_x100(60.0 / er.sqrt() * ratio.ln())
}
