//! What a copper pour becomes, once the board it sits on is taken into account.
//!
//! A zone as written is a rectangle and a net. The copper it turns into is that
//! rectangle minus every other piece of copper on the layer, each grown by the
//! clearance the fabricator demands - and minus a smaller gap around the pads
//! it is meant to reach, bridged by thermal spokes so those pads can still be
//! soldered.
//!
//! This lives beside the world rather than in the exporter because two places
//! need the same answer. The exporter writes those rectangles into Gerber, and
//! the viewer draws them on screen. When the two computed it separately, the
//! screen showed a plain rectangle and the fabricator got copper cut around
//! every pad - the same board, disagreeing with itself.

use cypcb_core::pour::{self, PourOptions};
use cypcb_core::{Nm, Point, Rect};

use crate::components::trace::{Trace, Via};
use crate::components::zone::{Hatch, Zone, ZoneOutline};
use crate::components::{same_net, BoardEdge, KeepoutKind, Layer, NetId};
use crate::footprint::FootprintLibrary;
use crate::world::BoardWorld;

/// The copper one pour becomes, as the rectangles that get made.
///
/// Split so a caller can tell the plane from the bridges into it: the exporter
/// emits both as regions, and a viewer may want to draw them differently.
#[derive(Debug, Clone, Default)]
pub struct FilledPour {
    /// The plane itself, cut around foreign copper and around its own pads.
    pub pieces: Vec<Rect>,
    /// Thermal spokes, clipped to the zone and to the copper they may occupy.
    pub spokes: Vec<Rect>,
    /// The plane of a pour drawn as a polygon: convex pieces, a ring of corners
    /// each. Empty for a rectangle, whose plane is `pieces`.
    pub shapes: Vec<Vec<Point>>,
    /// The spokes of a pour drawn as a polygon, cut to its ring.
    pub spoke_shapes: Vec<Vec<Point>>,
}

impl FilledPour {
    /// Every rectangle in this pour, plane and spokes alike.
    pub fn all(&self) -> impl Iterator<Item = &Rect> {
        self.pieces.iter().chain(self.spokes.iter())
    }

    /// Every convex piece of a polygon pour, plane and spokes alike.
    pub fn all_shapes(&self) -> impl Iterator<Item = &Vec<Point>> {
        self.shapes.iter().chain(self.spoke_shapes.iter())
    }
}

/// The copper pours on a layer that are drawn as a polygon, in the order the
/// board holds them.
///
/// A pour that carries a [`Hatch`] is left out: a mesh is cut from rectangles
/// and nothing here cuts one from a ring, and filling it solid would put copper
/// where a mesh was asked for. The importer never gives an outlined zone a
/// hatch and the language has no polygon, so no board reaches this today; the
/// refusal is here so that one that does is not drawn as a sheet.
///
/// A pour whose outline crosses itself is left out too, for the reason the
/// importer refuses one: it has no one area to cut copper from.
pub fn outlined_pours(
    world: &mut BoardWorld,
    layer: Layer,
) -> Vec<(bevy_ecs::entity::Entity, Zone, ZoneOutline)> {
    let mask = layer.to_copper_mask();
    let rows: Vec<_> = world
        .outlined_zones()
        .into_iter()
        .filter(|(_, zone, outline)| {
            zone.is_copper_pour() && zone.layer_mask & mask != 0 && outline.is_simple()
        })
        .collect();
    rows.into_iter()
        .filter(|(entity, _, _)| world.ecs().get::<Hatch>(*entity).is_none())
        .collect()
}

/// The areas that keep a pour out of this layer: every keepout that forbids
/// copper pour, as convex pieces. The five-kind keepout is one of them, as it
/// is for KiCad.
///
/// The fill is cut away from the area's outline with no gap, as KiCad's zone
/// filler cuts it (`zone_filler.cpp`, "Keepouts use outline with no
/// clearance"): the clearance a pour keeps is from copper, and a rule area is
/// not copper.
pub fn pour_cuts(world: &mut BoardWorld, layer: Layer) -> Vec<Vec<Point>> {
    use cypcb_core::pour_shape;

    let mask = layer.to_copper_mask();
    let mut cuts = Vec::new();
    for (entity, zone, outline) in world.all_zones() {
        if zone.layer_mask & mask == 0 || !world.forbids(entity, &zone, KeepoutKind::Pour) {
            continue;
        }
        match outline {
            Some(outline) => cuts.extend(pour_shape::convex_pieces(outline.corners())),
            None => cuts.push(vec![
                zone.bounds.min,
                Point::new(zone.bounds.max.x, zone.bounds.min.y),
                zone.bounds.max,
                Point::new(zone.bounds.min.x, zone.bounds.max.y),
            ]),
        }
    }
    cuts
}

/// The rectangle a convex piece is, when it is one that sits square to the axes.
fn as_rect(piece: &[Point]) -> Option<Rect> {
    let bounds = cypcb_core::pour_shape::shape_bounds(piece);
    let box_twice_area = 2
        * i128::from(bounds.max.x.0 - bounds.min.x.0)
        * i128::from(bounds.max.y.0 - bounds.min.y.0);
    (piece.len() == 4 && cypcb_core::pour_shape::twice_area(piece).abs() == box_twice_area)
        .then_some(bounds)
}

/// A rectangle as the ring of its corners, counter-clockwise.
fn ring_of(rect: Rect) -> Vec<Point> {
    vec![
        rect.min,
        Point::new(rect.max.x, rect.min.y),
        rect.max,
        Point::new(rect.min.x, rect.max.y),
    ]
}

/// Where a pour is held: inside the board, outside every cutout, and the
/// fab's edge distance from every edge there is.
///
/// KiCad clips the fill to the board outline with its holes
/// (`zone_filler.cpp:409-410`, `zone.cpp:1558-1564`) and knocks every
/// Edge.Cuts shape out of it - the outline and each cutout alike - by the edge
/// clearance (`zone_filler.cpp:2172-2180`). This is the same, on the edge the
/// checks and the files already read: [`BoardEdge`].
struct BoardHold {
    edge: BoardEdge,
    gap: Nm,
    /// The board's own rectangle shrunk by the gap, when the board is one
    /// rectangle with nothing cut out of it. Every cut is then a rectangle cut
    /// and a pour made of rectangles stays made of them.
    inset: Option<Option<Rect>>,
    outer: Vec<Vec<Point>>,
    cutouts: Vec<Vec<Vec<Point>>>,
    /// Every edge of every ring, and the corners where a disc is needed.
    edges: Vec<(Point, Point)>,
    corners: Vec<Point>,
}

impl BoardHold {
    /// The hold of this board, or `None` where the board has no size: a file
    /// whose outline never closed is a board with no edge to hold a pour to.
    fn of(world: &BoardWorld, gap: Nm) -> Option<BoardHold> {
        use cypcb_core::pour_shape;

        let (size, _) = world.board_info()?;
        if size.width.0 <= 0 || size.height.0 <= 0 {
            return None;
        }
        let edge = BoardEdge::of(world).unwrap_or_else(|| {
            BoardEdge::new(
                ring_of(Rect::new(
                    Point::ORIGIN,
                    Point::new(size.width, size.height),
                )),
                Vec::new(),
            )
        });
        let inset = if edge.cutouts().is_empty() {
            as_rect(edge.outer()).map(|bounds| {
                let (lo, hi) = (
                    Point::new(Nm(bounds.min.x.0 + gap.0), Nm(bounds.min.y.0 + gap.0)),
                    Point::new(Nm(bounds.max.x.0 - gap.0), Nm(bounds.max.y.0 - gap.0)),
                );
                (lo.x.0 < hi.x.0 && lo.y.0 < hi.y.0).then(|| Rect::new(lo, hi))
            })
        } else {
            None
        };
        let outer = pour_shape::convex_pieces(edge.outer());
        let cutouts = edge
            .cutouts()
            .iter()
            .map(|ring| pour_shape::convex_pieces(ring))
            .collect();
        let edges: Vec<(Point, Point)> = edge.edges().collect();

        // A disc is needed where the board turns away from the copper: the
        // band round each side leaves a wedge there that is nearer than the
        // gap. With the board on the left of every ring, that is a right turn.
        let mut corners = Vec::new();
        for (index, ring) in edge.rings().enumerate() {
            let board_on_left = (pour_shape::twice_area(ring) > 0) == (index == 0);
            for i in 0..ring.len() {
                let (before, at, after) = (
                    ring[(i + ring.len() - 1) % ring.len()],
                    ring[i],
                    ring[(i + 1) % ring.len()],
                );
                let turn = (i128::from(at.x.0) - i128::from(before.x.0))
                    * (i128::from(after.y.0) - i128::from(at.y.0))
                    - (i128::from(at.y.0) - i128::from(before.y.0))
                        * (i128::from(after.x.0) - i128::from(at.x.0));
                if (turn < 0) == board_on_left && turn != 0 {
                    corners.push(at);
                }
            }
        }
        Some(BoardHold {
            edge,
            gap,
            inset,
            outer,
            cutouts,
            edges,
            corners,
        })
    }

    /// One convex piece of a pour, held to the board.
    ///
    /// A piece that lies wholly inside and no nearer than the gap to any edge
    /// comes back as it is; one wholly outside, or inside a cutout, is gone.
    fn hold_piece(&self, piece: &[Point]) -> Vec<Vec<Point>> {
        use cypcb_core::pour_shape;

        let near: Vec<(Point, Point)> = self
            .edges
            .iter()
            .copied()
            .filter(|(a, b)| pour_shape::comes_within(piece, *a, *b, self.gap))
            .collect();
        if near.is_empty() {
            return if self.edge.contains(piece[0]) {
                vec![piece.to_vec()]
            } else {
                Vec::new()
            };
        }

        let mut pieces: Vec<Vec<Point>> = self
            .outer
            .iter()
            .filter_map(|part| pour_shape::intersect_convex(piece, part))
            .collect();
        for cutout in self.cutouts.iter().flatten() {
            pieces = pour_shape::cut_pieces_by(pieces, cutout);
        }
        for (a, b) in &near {
            if let Some(band) = pour_shape::segment_band(*a, *b, self.gap) {
                pieces = pour_shape::cut_pieces_by(pieces, &band);
            }
        }
        for corner in &self.corners {
            if near.iter().any(|(a, b)| a == corner || b == corner) {
                pieces =
                    pour_shape::cut_pieces_by(pieces, &pour_shape::disc_around(*corner, self.gap));
            }
        }
        pieces
    }

    /// A pour made of rectangles, held to the board: the rectangles that are
    /// still rectangles, and the convex pieces of the ones that are not.
    fn hold_rects(&self, rects: Vec<Rect>) -> (Vec<Rect>, Vec<Vec<Point>>) {
        if let Some(inset) = self.inset {
            let kept = inset
                .map(|inset| {
                    rects
                        .iter()
                        .filter_map(|r| pour::intersect(*r, inset))
                        .collect()
                })
                .unwrap_or_default();
            return (kept, Vec::new());
        }
        let mut kept = Vec::new();
        let mut shapes = Vec::new();
        for rect in rects {
            let ring = ring_of(rect);
            let held = self.hold_piece(&ring);
            if held.len() == 1 && held[0] == ring {
                kept.push(rect);
            } else {
                shapes.extend(held);
            }
        }
        (kept, shapes)
    }

    /// A pour made of convex pieces, held to the board.
    fn hold_shapes(&self, shapes: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
        if let Some(inset) = self.inset {
            return shapes
                .into_iter()
                .filter_map(|piece| {
                    let bounds = cypcb_core::pour_shape::shape_bounds(&piece);
                    let inset = inset?;
                    if bounds.min.x.0 >= inset.min.x.0
                        && bounds.min.y.0 >= inset.min.y.0
                        && bounds.max.x.0 <= inset.max.x.0
                        && bounds.max.y.0 <= inset.max.y.0
                    {
                        Some(piece)
                    } else {
                        cypcb_core::pour_shape::clip_to_rect(&piece, inset)
                    }
                })
                .collect();
        }
        shapes
            .into_iter()
            .flat_map(|piece| self.hold_piece(&piece))
            .collect()
    }
}

/// Fill one pour that is drawn as a polygon against the board it sits on.
///
/// The same pour as [`fill_zone`] makes of a rectangle, cut from the ring and
/// not from its box: the ring minus every foreign piece of copper grown by the
/// clearance, minus the thermal gap round the pads of its own net, bridged by
/// spokes cut to the ring.
pub fn fill_outlined_zone(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    zone: &Zone,
    outline: &ZoneOutline,
    options: &PourOptions,
) -> FilledPour {
    use cypcb_core::pour_shape;

    let (obstacles, own_pads) = copper_on_layer(world, library, layer, zone.net);

    let mut shapes = pour_shape::fill_ring(outline.corners(), &obstacles, options.clearance);
    for pad in &own_pads {
        shapes = pour_shape::cut_pieces(shapes, pour::grown(*pad, options.thermal_gap));
    }

    let ring = pour_shape::convex_pieces(outline.corners());
    let mut spoke_shapes = Vec::new();
    for pad in &own_pads {
        for spoke in pour::thermal_spokes(*pad, options) {
            let inside: Vec<Vec<Point>> = ring
                .iter()
                .filter_map(|piece| pour_shape::clip_to_rect(piece, spoke))
                .collect();
            spoke_shapes.extend(pour_shape::cut_all(inside, &obstacles, options.clearance));
        }
    }

    // The rule areas that keep a pour out, taken out of the plane and its
    // bridges alike.
    for cut in pour_cuts(world, layer) {
        shapes = pour_shape::cut_pieces_by(shapes, &cut);
        spoke_shapes = pour_shape::cut_pieces_by(spoke_shapes, &cut);
    }

    // And the board's own edge, and its cutouts.
    if let Some(hold) = BoardHold::of(world, options.edge_clearance) {
        shapes = hold.hold_shapes(shapes);
        spoke_shapes = hold.hold_shapes(spoke_shapes);
    }

    FilledPour {
        shapes,
        spoke_shapes,
        ..FilledPour::default()
    }
}

/// Fill one zone against the board it sits on.
///
/// `layer` is the copper layer being filled; a zone spanning several layers is
/// filled once per layer, because what obstructs it differs on each.
pub fn fill_zone(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    zone: &Zone,
    hatch: Option<Hatch>,
    options: &PourOptions,
) -> FilledPour {
    let (obstacles, own_pads) = copper_on_layer(world, library, layer, zone.net);

    // The clearance applies to the foreign copper only. A pad on the pour's own
    // net is cut with the thermal gap instead and then bridged, so the joint
    // can be soldered: solid copper carries heat away from a pin faster than an
    // iron can put it in.
    let mut pieces = pour::fill(zone.bounds, &obstacles, options.clearance);
    for pad in &own_pads {
        let keepout = pour::grown(*pad, options.thermal_gap);
        pieces = pieces
            .into_iter()
            .flat_map(|piece| pour::fill(piece, &[keepout], Nm::ZERO))
            .collect();
    }

    // Put the spokes back, cut to the copper they are allowed to occupy.
    //
    // A spoke reaches a quarter of a millimetre past its pad, which on a dense
    // board is far enough to cross into a neighbour's clearance - measured on
    // two 0402s a millimetre apart, where the horizontal bar ran from 6.946mm
    // to 8.054mm and the foreign keepout started at 7.9mm. Subtracting the same
    // obstacles the pour was cut against keeps the bridge inside the plane it
    // bridges to.
    let mut spokes = Vec::new();
    for pad in &own_pads {
        for spoke in pour::thermal_spokes(*pad, options) {
            let Some(spoke) = pour::intersect(spoke, zone.bounds) else {
                continue;
            };
            spokes.extend(pour::fill(spoke, &obstacles, options.clearance));
        }
    }

    // The rule areas that keep a pour out. One that sits square to the axes is
    // taken out of the rectangles as another obstacle would be, with no gap.
    let (square, slanted): (Vec<Vec<Point>>, Vec<Vec<Point>>) = pour_cuts(world, layer)
        .into_iter()
        .partition(|cut| as_rect(cut).is_some());
    for hole in square.iter().filter_map(|cut| as_rect(cut)) {
        pieces = pieces
            .into_iter()
            .flat_map(|piece| pour::fill(piece, &[hole], Nm::ZERO))
            .collect();
        spokes = spokes
            .into_iter()
            .flat_map(|spoke| pour::fill(spoke, &[hole], Nm::ZERO))
            .collect();
    }

    // A mesh rather than a sheet, when the pour asked for one.
    //
    // Cut last, against the copper the plane already became: hatching first
    // and then subtracting obstacles would give the same answer more slowly,
    // and hatching the spokes would cut the bridges a pad is soldered through.
    // A spoke is 0.25mm of copper reaching a pin; there is nothing in it to
    // relieve.
    if let Some(hatch) = hatch {
        pieces = hatched(&pieces, hatch);
    }

    // The board's own edge and its cutouts. A rectangle that meets none of
    // them stays a rectangle; one that does and is cut at a slant comes back
    // as the convex pieces a polygon pour is made of.
    let (mut held_shapes, mut held_spoke_shapes) = (Vec::new(), Vec::new());
    if let Some(hold) = BoardHold::of(world, options.edge_clearance) {
        (pieces, held_shapes) = hold.hold_rects(pieces);
        (spokes, held_spoke_shapes) = hold.hold_rects(spokes);
    }

    // A rule area that is not square to the axes leaves copper that is not a
    // rectangle. The plane and its bridges then come back as the convex pieces
    // a polygon pour is made of, and the rectangles are empty.
    if !slanted.is_empty() {
        let mut shapes: Vec<Vec<Point>> = pieces.iter().copied().map(ring_of).collect();
        let mut spoke_shapes: Vec<Vec<Point>> = spokes.iter().copied().map(ring_of).collect();
        shapes.extend(held_shapes);
        spoke_shapes.extend(held_spoke_shapes);
        for cut in &slanted {
            shapes = cypcb_core::pour_shape::cut_pieces_by(shapes, cut);
            spoke_shapes = cypcb_core::pour_shape::cut_pieces_by(spoke_shapes, cut);
        }
        return FilledPour {
            shapes,
            spoke_shapes,
            ..FilledPour::default()
        };
    }

    FilledPour {
        pieces,
        spokes,
        shapes: held_shapes,
        spoke_shapes: held_spoke_shapes,
    }
}

/// The copper a hatch leaves of a solid fill: lines of `width`, centre to
/// centre at `pitch`, running both ways.
///
/// Measured from the origin rather than from each piece, so two pieces of one
/// plane are cut by the same mesh and the lines line up across a pad that
/// splits the pour in two.
///
/// A pitch no larger than the width is a mesh with no gaps in it, which is a
/// sheet: the pieces come back untouched rather than as a plane cut into
/// lines that touch each other. The checker is where a design that states such
/// a hatch gets told, not here.
fn hatched(pieces: &[Rect], hatch: Hatch) -> Vec<Rect> {
    if hatch.width.0 <= 0 || hatch.pitch.0 <= hatch.width.0 {
        return pieces.to_vec();
    }

    let mut out = Vec::new();
    for piece in pieces {
        // Every line of the mesh that reaches this piece, in both directions.
        // The first line at or before the piece's own edge, so the mesh is the
        // board's rather than the piece's.
        for (horizontal, low, high) in [
            (true, piece.min.y.0, piece.max.y.0),
            (false, piece.min.x.0, piece.max.x.0),
        ] {
            let first = low.div_euclid(hatch.pitch.0) * hatch.pitch.0;
            let mut at = first;
            while at <= high {
                let start = at.max(low);
                let end = (at + hatch.width.0).min(high);
                at += hatch.pitch.0;
                if end <= start {
                    continue;
                }
                let line = if horizontal {
                    Rect::new(
                        Point::new(piece.min.x, Nm(start)),
                        Point::new(piece.max.x, Nm(end)),
                    )
                } else {
                    Rect::new(
                        Point::new(Nm(start), piece.min.y),
                        Point::new(Nm(end), piece.max.y),
                    )
                };
                out.push(line);
            }
        }
    }
    out
}

/// Every piece of copper on this layer that a pour on `pour_net` must keep away
/// from, and separately the pads it is meant to reach.
///
/// Copper on the pour's own net is left out of the first list: the pour is that
/// net, and keeping clear of it would leave the plane unconnected to the thing
/// it grounds.
pub fn copper_on_layer(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    pour_net: Option<NetId>,
) -> (Vec<Rect>, Vec<Rect>) {
    use crate::components::{
        place_box, place_pad, FootprintRef, NetConnections, Position, Rotation,
    };

    let mut boxes = Vec::new();
    let mut own = Vec::new();

    /// A placed part as this function needs it: where it is, how it is turned,
    /// which footprint it wears and what its pins are wired to.
    type Placement = (Point, f64, String, Vec<(String, NetId)>);

    let placements: Vec<Placement> = {
        let ecs = world.ecs_mut();
        crate::in_build_order::<(&Position, &Rotation, &FootprintRef, Option<&NetConnections>)>(ecs)
            .into_iter()
            .map(|(position, rotation, footprint, nets)| {
                let pins = nets
                    .map(|n| n.iter().map(|p| (p.pin.clone(), p.net)).collect())
                    .unwrap_or_default();
                (
                    position.0,
                    rotation.to_degrees(),
                    footprint.as_str().to_string(),
                    pins,
                )
            })
            .collect()
    };

    for (position, degrees, name, pins) in placements {
        let Some(footprint) = library.get(&name) else {
            continue;
        };
        let rotation = Rotation::from_degrees(degrees);

        for pad in &footprint.pads {
            if !pad.is_on(layer) {
                continue;
            }
            let pad_net = pins
                .iter()
                .find(|(pin, _)| *pin == pad.number)
                .map(|(_, net)| *net);
            let is_own = pad_net.is_some() && pad_net == pour_net;

            let (half_w, half_h) = (pad.size.0 .0 / 2, pad.size.1 .0 / 2);
            let box_ = place_box(
                place_pad(position, pad.position, rotation),
                Rect {
                    min: Point::new(Nm(-half_w), Nm(-half_h)),
                    max: Point::new(Nm(half_w), Nm(half_h)),
                },
                // The pad turns about its own centre by its part's turn and
                // its own together.
                Rotation(rotation.0 + pad.rotation.0),
            );
            if is_own {
                own.push(box_);
            } else {
                boxes.push(box_);
            }
        }
    }

    // Traces and vias.
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        crate::in_build_order::<&Trace>(ecs)
            .into_iter()
            .cloned()
            .collect()
    };
    for trace in traces {
        if trace.layer != layer || same_net(trace.net_id, pour_net) {
            continue;
        }
        let half = trace.width.0 / 2;
        for segment in &trace.segments {
            boxes.push(Rect {
                min: Point::new(
                    Nm(segment.start.x.0.min(segment.end.x.0) - half),
                    Nm(segment.start.y.0.min(segment.end.y.0) - half),
                ),
                max: Point::new(
                    Nm(segment.start.x.0.max(segment.end.x.0) + half),
                    Nm(segment.start.y.0.max(segment.end.y.0) + half),
                ),
            });
        }
    }

    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        crate::in_build_order::<&Via>(ecs)
            .into_iter()
            .copied()
            .collect()
    };
    for via in vias {
        if same_net(via.net_id, pour_net) {
            continue;
        }
        let radius = via.outer_diameter.0 / 2;
        boxes.push(Rect {
            min: Point::new(Nm(via.position.x.0 - radius), Nm(via.position.y.0 - radius)),
            max: Point::new(Nm(via.position.x.0 + radius), Nm(via.position.y.0 + radius)),
        });
    }

    (boxes, own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::zone::{Zone, ZoneKind};
    use crate::components::PinConnection;
    use crate::components::{FootprintRef, Layer, NetId, PadShape, Position, Rotation};
    use crate::footprint::{Footprint, PadDef};
    use crate::{NetConnections, RefDes, Value};

    fn board_with_one_pad(pad_net: u32) -> (BoardWorld, FootprintLibrary) {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

        let mut library = FootprintLibrary::new();
        library.register(
            Footprint {
                placement_origin: None,
                name: "PAD1".into(),
                description: String::new(),
                bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
                courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
                silk: Vec::new(),
                pads: vec![PadDef {
                    hole_offset: None,
                    number: "1".into(),
                    shape: PadShape::Rect,
                    position: Point::ORIGIN,
                    size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                    drill: None,
                    slot: None,
                    layers: vec![Layer::TopCopper],
                    mask_margin: None,
                    rotation: Rotation::ZERO,
                }],
            }
            .with_courtyard_holding_its_pads(),
        );

        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), NetId::new(pad_net)));
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("1k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );

        (world, library)
    }

    fn ground_zone() -> Zone {
        Zone {
            bounds: Rect {
                min: Point::from_mm(5.0, 5.0),
                max: Point::from_mm(15.0, 15.0),
            },
            kind: ZoneKind::CopperPour,
            layer_mask: Layer::TopCopper.to_copper_mask(),
            name: None,
            net: Some(NetId::new(1)),
        }
    }

    #[test]
    fn a_pad_on_a_foreign_net_is_cut_out_of_the_plane() {
        let (mut world, library) = board_with_one_pad(2);
        let filled = fill_zone(
            &mut world,
            &library,
            Layer::TopCopper,
            &ground_zone(),
            None,
            &PourOptions::default(),
        );

        // The pad sits in the middle of the zone, so cutting it leaves several
        // pieces and none of them may contain the pad's centre.
        assert!(
            filled.pieces.len() > 1,
            "a hole in the middle splits a plane"
        );
        let centre = Point::from_mm(10.0, 10.0);
        for piece in filled.all() {
            let inside = piece.min.x.0 <= centre.x.0
                && centre.x.0 <= piece.max.x.0
                && piece.min.y.0 <= centre.y.0
                && centre.y.0 <= piece.max.y.0;
            assert!(!inside, "copper covers a pad on another net");
        }
        assert!(
            filled.spokes.is_empty(),
            "a foreign pad gets no bridge to the plane"
        );
    }

    #[test]
    fn a_pad_on_the_pours_own_net_gets_bridged_to_it() {
        let (mut world, library) = board_with_one_pad(1);
        let filled = fill_zone(
            &mut world,
            &library,
            Layer::TopCopper,
            &ground_zone(),
            None,
            &PourOptions::default(),
        );

        assert!(
            !filled.spokes.is_empty(),
            "a pad on the pour's own net is connected by thermal spokes"
        );
    }

    /// The copper a pour keeps clear of, for one 1.0 by 2.0 pad at the origin
    /// of a part at (10, 10): the part turned `part`, the pad `pad`.
    fn pad_boxes(part: Rotation, pad: Rotation) -> (Vec<Rect>, Vec<Rect>) {
        let (mut world, mut library) = board_with_one_pad(1);
        let mut footprint = library.get("PAD1").unwrap().clone();
        footprint.pads[0].size = (Nm::from_mm(1.0), Nm::from_mm(2.0));
        footprint.pads[0].rotation = pad;
        library.register(footprint);
        for mut rotation in world
            .ecs_mut()
            .query::<&mut Rotation>()
            .iter_mut(&mut *world.ecs_mut())
        {
            *rotation = part;
        }
        copper_on_layer(&mut world, &library, Layer::TopCopper, None)
    }

    #[test]
    fn a_pad_turned_in_its_footprint_keeps_the_pour_off_as_its_part_turned() {
        let turned_pad = pad_boxes(Rotation::ZERO, Rotation::DEG_90);
        assert_eq!(turned_pad, pad_boxes(Rotation::DEG_90, Rotation::ZERO));
        assert_ne!(
            turned_pad,
            pad_boxes(Rotation::ZERO, Rotation::ZERO),
            "the control"
        );
    }
    /// How many pieces of copper `pour_net` keeps off, with one 10mm trace
    /// and one via added on `copper_net`.
    fn boxes_with_copper_on(copper_net: Option<NetId>, pour_net: Option<NetId>) -> usize {
        use crate::components::trace::{Trace, TraceSegment, Via};
        let (mut world, library) = board_with_one_pad(1);
        let before = copper_on_layer(&mut world, &library, Layer::TopCopper, pour_net)
            .0
            .len();
        let mut trace = Trace::new(NetId::new(0));
        trace.net_id = copper_net;
        trace.add_segment(TraceSegment::new(
            Point::from_mm(2.0, 2.0),
            Point::from_mm(8.0, 2.0),
        ));
        world.spawn_entity((trace,));
        let mut via = Via::new(Point::from_mm(4.0, 4.0), NetId::new(0));
        via.net_id = copper_net;
        world.spawn_entity((via,));
        copper_on_layer(&mut world, &library, Layer::TopCopper, pour_net)
            .0
            .len()
            - before
    }

    #[test]
    fn a_pour_keeps_off_copper_of_another_net_and_not_its_own() {
        assert_eq!(
            boxes_with_copper_on(Some(NetId::new(2)), Some(NetId::new(1))),
            2
        );
        assert_eq!(
            boxes_with_copper_on(Some(NetId::new(1)), Some(NetId::new(1))),
            0
        );
    }

    #[test]
    fn copper_on_no_net_is_never_the_pours_own() {
        // A pour on no net and copper on no net are not the same net, so the
        // pour keeps off it, as it keeps off copper on any other net. Written
        // with `==` the pair read as equal and the pour poured over it.
        assert_eq!(boxes_with_copper_on(None, None), 2);
        // And a pour on a net keeps off copper on none.
        assert_eq!(boxes_with_copper_on(None, Some(NetId::new(1))), 2);
    }
}
