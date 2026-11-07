//! Structured vector schematic data model.
//!
//! Conventions:
//! - Geometry is in millimeters with the Y axis pointing DOWN (standard screen & KiCad schematic orientation).
//! - Coordinates in `SchSheetDef` are absolute on that sheet.
//! - Parsers resolve symbol placement, rotation, mirroring and graphics so the renderer
//!   and connectivity engine never repeat that math.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchematicFormat {
    KiCadSch,
    KiCadLegacySch,
    EagleSch,
    AltiumSch,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SchPoint {
    pub x: f64,
    pub y: f64,
}

impl SchPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl SchBounds {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self { min_x, min_y, max_x, max_y }
    }

    pub fn empty() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min_x > self.max_x || self.min_y > self.max_y
    }

    pub fn union_point(&mut self, pt: SchPoint) {
        if pt.x < self.min_x { self.min_x = pt.x; }
        if pt.x > self.max_x { self.max_x = pt.x; }
        if pt.y < self.min_y { self.min_y = pt.y; }
        if pt.y > self.max_y { self.max_y = pt.y; }
    }

    pub fn union_bounds(&mut self, other: &Self) {
        if other.is_empty() { return; }
        if other.min_x < self.min_x { self.min_x = other.min_x; }
        if other.max_x > self.max_x { self.max_x = other.max_x; }
        if other.min_y < self.min_y { self.min_y = other.min_y; }
        if other.max_y > self.max_y { self.max_y = other.max_y; }
    }

    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(0.0)
    }

    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(0.0)
    }

    pub fn center(&self) -> SchPoint {
        SchPoint::new((self.min_x + self.max_x) * 0.5, (self.min_y + self.max_y) * 0.5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchPinType {
    Input,
    Output,
    Bidirectional,
    TriState,
    Passive,
    Free,
    #[default]
    Unspecified,
    PowerIn,
    PowerOut,
    OpenCollector,
    OpenEmitter,
    NoConnect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchPin {
    pub id: String,
    pub number: String,
    pub name: String,
    pub at: SchPoint,
    pub body: SchPoint,
    pub pin_type: SchPinType,
    pub hidden: bool,
    pub unit: u32,
    pub implicit_net: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchGraphic {
    Polyline {
        points: Vec<SchPoint>,
        width: f64,
        filled: bool,
    },
    Rect {
        min: SchPoint,
        max: SchPoint,
        width: f64,
        filled: bool,
    },
    Circle {
        center: SchPoint,
        radius: f64,
        width: f64,
        filled: bool,
    },
    Text {
        at: SchPoint,
        text: String,
        angle: f64,
        size: f64,
        hidden: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchField {
    pub name: String,
    pub value: String,
    pub at: Option<SchPoint>,
    pub angle: f64,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchSymbol {
    pub id: String,
    pub lib_id: String,
    pub ref_default: String,
    pub instances: BTreeMap<String, String>, // instance_path -> annotated_ref
    pub value: String,
    pub footprint: String,
    pub unit: u32,
    pub unit_count: u32,
    pub at: SchPoint,
    pub rotation: f64,
    pub mirror_x: bool,
    pub mirror_y: bool,
    pub pins: Vec<SchPin>,
    pub graphics: Vec<SchGraphic>,
    pub fields: Vec<SchField>,
    pub power_net: Option<String>,
    pub virtual_symbol: bool,
    pub dnp: bool,
    pub bounds: SchBounds,
}

impl SchSymbol {
    pub fn ref_for_instance(&self, instance_path: &str) -> &str {
        if let Some(r) = self.instances.get(instance_path) {
            r
        } else if let Some(r) = self.instances.get("") {
            r
        } else {
            &self.ref_default
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchWire {
    pub id: String,
    pub a: SchPoint,
    pub b: SchPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchBus {
    pub id: String,
    pub a: SchPoint,
    pub b: SchPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchBusEntry {
    pub id: String,
    pub at: SchPoint,
    pub to: SchPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchJunction {
    pub id: String,
    pub at: SchPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchNoConnect {
    pub id: String,
    pub at: SchPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchLabelKind {
    Local,
    Global,
    Hierarchical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchLabel {
    pub id: String,
    pub kind: SchLabelKind,
    pub text: String,
    pub at: SchPoint,
    pub angle: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchSheetDef {
    pub id: String,
    pub name: String,
    pub file: String,
    pub title: String,
    pub symbols: Vec<SchSymbol>,
    pub wires: Vec<SchWire>,
    pub buses: Vec<SchBus>,
    pub bus_entries: Vec<SchBusEntry>,
    pub junctions: Vec<SchJunction>,
    pub no_connects: Vec<SchNoConnect>,
    pub labels: Vec<SchLabel>,
    pub graphics: Vec<SchGraphic>,
    pub bounds: SchBounds,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchSheetInstance {
    pub path: String,
    pub def_id: String,
    pub name: String,
    pub page: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchDeclaredNet {
    pub name: String,
    pub pins: Vec<SchNetPinRef>,
    pub wire_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchNetPinRef {
    pub instance_path: String,
    pub def_id: String,
    pub symbol_id: String,
    pub pin_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchNetMember {
    pub instance_path: String,
    pub def_id: String,
    pub symbol_id: String,
    pub pin_id: String,
    pub ref_des: String,
    pub pin_number: String,
    pub pin_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchNet {
    pub id: String,
    pub name: String,
    pub auto: bool,
    pub aliases: Vec<String>,
    pub members: Vec<SchNetMember>,
    pub wire_ids: Vec<(String, String)>, // (def_id/instance_path, wire_id)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Schematic {
    pub format: SchematicFormat,
    pub format_label: String,
    pub name: String,
    pub defs: Vec<SchSheetDef>,
    pub root_def_id: String,
    pub instances: Vec<SchSheetInstance>,
    pub declared_nets: Vec<SchDeclaredNet>,
    pub nets: Vec<SchNet>,
    pub diagnostics: Vec<SchDiagnostic>,
}

impl Schematic {
    pub fn root_sheet(&self) -> Option<&SchSheetDef> {
        self.defs.iter().find(|d| d.id == self.root_def_id)
    }

    pub fn find_sheet(&self, id: &str) -> Option<&SchSheetDef> {
        self.defs.iter().find(|d| d.id == id)
    }

    pub fn find_symbol_by_ref(&self, ref_des: &str) -> Option<(&SchSheetDef, &SchSymbol)> {
        for def in &self.defs {
            for sym in &def.symbols {
                if sym.ref_default.eq_ignore_ascii_case(ref_des)
                    || sym.instances.values().any(|r| r.eq_ignore_ascii_case(ref_des))
                {
                    return Some((def, sym));
                }
            }
        }
        None
    }

    pub fn find_net_by_name(&self, net_name: &str) -> Option<&SchNet> {
        self.nets.iter().find(|n| {
            n.name.eq_ignore_ascii_case(net_name)
                || n.aliases.iter().any(|a| a.eq_ignore_ascii_case(net_name))
        })
    }
}
