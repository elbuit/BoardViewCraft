//! Schematic module for native vector CAD schematics (KiCad, EAGLE, etc.)

pub mod model;
pub mod connectivity;
pub mod eagle_sch;
pub mod kicad_sch;

pub use model::*;
pub use eagle_sch::parse_eagle_sch;
pub use kicad_sch::parse_kicad_sch;
pub use connectivity::solve_connectivity;
