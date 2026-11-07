//! Connectivity solver for schematics without explicit net definitions (e.g. KiCad).
//!
//! Connects wires, pins, junctions, and labels based on 2D geometric coincidence.
//! Tolerance: 0.001 mm (1 µm).

use std::collections::{HashMap, HashSet};
use super::model::*;

const TOLERANCE: f64 = 0.001;

/// Spatial hash key for 2D points to find coincidence.
fn quantize_point(pt: SchPoint) -> (i64, i64) {
    let scale = 1.0 / TOLERANCE;
    ((pt.x * scale).round() as i64, (pt.y * scale).round() as i64)
}

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while root != self.parent[root] {
            root = self.parent[root];
        }
        let mut curr = i;
        while curr != root {
            let next = self.parent[curr];
            self.parent[curr] = root;
            curr = next;
        }
        root
    }

    fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);
        if root_i != root_j {
            self.parent[root_i] = root_j;
        }
    }
}

enum ConductorNode<'a> {
    WireEnd { wire_idx: usize, pt: SchPoint },
    PinTip { sym_idx: usize, pt: SchPoint, pin: &'a SchPin },
    Label { pt: SchPoint, label: &'a SchLabel },
}

impl<'a> ConductorNode<'a> {
    fn point(&self) -> SchPoint {
        match self {
            ConductorNode::WireEnd { pt, .. } => *pt,
            ConductorNode::PinTip { pt, .. } => *pt,
            ConductorNode::Label { pt, .. } => *pt,
        }
    }
}

/// Compute net connectivity for a schematic.
pub fn solve_connectivity(schematic: &mut Schematic) {
    // If the schematic has declared nets (e.g. EAGLE), we build SchNets from them
    if !schematic.declared_nets.is_empty() {
        let mut nets = Vec::new();
        for decl in &schematic.declared_nets {
            let mut members = Vec::new();
            for p in &decl.pins {
                if let Some(sheet) = schematic.find_sheet(&p.def_id) {
                    if let Some(sym) = sheet.symbols.iter().find(|s| s.id == p.symbol_id) {
                        if let Some(pin) = sym.pins.iter().find(|pin| pin.id == p.pin_id) {
                            members.push(SchNetMember {
                                instance_path: p.instance_path.clone(),
                                def_id: p.def_id.clone(),
                                symbol_id: p.symbol_id.clone(),
                                pin_id: p.pin_id.clone(),
                                ref_des: sym.ref_for_instance(&p.instance_path).to_string(),
                                pin_number: pin.number.clone(),
                                pin_name: pin.name.clone(),
                            });
                        }
                    }
                }
            }

            let mut wire_ids = Vec::new();
            for wid in &decl.wire_ids {
                wire_ids.push((schematic.root_def_id.clone(), wid.clone()));
            }

            nets.push(SchNet {
                id: format!("net:{}", decl.name),
                name: decl.name.clone(),
                auto: false,
                aliases: vec![decl.name.clone()],
                members,
                wire_ids,
            });
        }
        schematic.nets = nets;
        return;
    }

    // Otherwise, geometric resolution per sheet
    let mut all_nets: Vec<SchNet> = Vec::new();

    for sheet in &schematic.defs {
        let mut nodes: Vec<ConductorNode> = Vec::new();

        // 1. Collect wire ends
        for (w_idx, w) in sheet.wires.iter().enumerate() {
            nodes.push(ConductorNode::WireEnd { wire_idx: w_idx, pt: w.a });
            nodes.push(ConductorNode::WireEnd { wire_idx: w_idx, pt: w.b });
        }

        // 2. Collect pin tips
        for (s_idx, sym) in sheet.symbols.iter().enumerate() {
            for pin in &sym.pins {
                nodes.push(ConductorNode::PinTip { sym_idx: s_idx, pt: pin.at, pin });
            }
        }

        // 3. Collect labels
        for label in &sheet.labels {
            nodes.push(ConductorNode::Label { pt: label.at, label });
        }

        if nodes.is_empty() {
            continue;
        }

        // Initialize DSU
        let mut dsu = DisjointSet::new(nodes.len());

        // Connect nodes sharing the same wire
        for i in 0..sheet.wires.len() {
            let node_a = i * 2;
            let node_b = i * 2 + 1;
            dsu.union(node_a, node_b);
        }

        // Spatial grid for coincident points
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (idx, node) in nodes.iter().enumerate() {
            let key = quantize_point(node.point());
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let neighbor = (key.0 + dx, key.1 + dy);
                    if let Some(others) = grid.get(&neighbor) {
                        for &other_idx in others {
                            if nodes[other_idx].point().distance_to(&node.point()) <= TOLERANCE {
                                dsu.union(idx, other_idx);
                            }
                        }
                    }
                }
            }
            grid.entry(key).or_default().push(idx);
        }

        // Also check junctions (connecting wires that cross or T-join)
        for junction in &sheet.junctions {
            let j_pt = junction.at;
            let mut touching_indices = Vec::new();
            for (idx, node) in nodes.iter().enumerate() {
                if node.point().distance_to(&j_pt) <= TOLERANCE {
                    touching_indices.push(idx);
                }
            }
            for i in 1..touching_indices.len() {
                dsu.union(touching_indices[0], touching_indices[i]);
            }
        }

        // Group into nets
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for idx in 0..nodes.len() {
            let root = dsu.find(idx);
            groups.entry(root).or_default().push(idx);
        }

        let mut auto_net_counter = 1;

        for (_root, member_indices) in groups {
            let mut wire_set = HashSet::new();
            let mut pins = Vec::new();
            let mut label_names = Vec::new();

            for &idx in &member_indices {
                match &nodes[idx] {
                    ConductorNode::WireEnd { wire_idx, .. } => {
                        wire_set.insert(&sheet.wires[*wire_idx].id);
                    }
                    ConductorNode::PinTip { sym_idx, pin, .. } => {
                        let sym = &sheet.symbols[*sym_idx];
                        pins.push(SchNetMember {
                            instance_path: "".to_string(),
                            def_id: sheet.id.clone(),
                            symbol_id: sym.id.clone(),
                            pin_id: pin.id.clone(),
                            ref_des: sym.ref_default.clone(),
                            pin_number: pin.number.clone(),
                            pin_name: pin.name.clone(),
                        });
                    }
                    ConductorNode::Label { label, .. } => {
                        label_names.push(label.text.clone());
                    }
                }
            }

            // Power symbols check
            for &idx in &member_indices {
                if let ConductorNode::PinTip { sym_idx, .. } = &nodes[idx] {
                    let sym = &sheet.symbols[*sym_idx];
                    if let Some(pwr) = &sym.power_net {
                        label_names.push(pwr.clone());
                    }
                }
            }

            label_names.sort();
            label_names.dedup();

            let (name, auto) = if let Some(first_label) = label_names.first() {
                (first_label.clone(), false)
            } else if let Some(first_pin) = pins.first() {
                (format!("Net-({}-Pad{})", first_pin.ref_des, first_pin.pin_number), true)
            } else {
                let n = format!("Net-Auto-{}", auto_net_counter);
                auto_net_counter += 1;
                (n, true)
            };

            let wire_ids = wire_set.into_iter().map(|w| (sheet.id.clone(), w.clone())).collect();

            all_nets.push(SchNet {
                id: format!("net:{}", name),
                name,
                auto,
                aliases: label_names,
                members: pins,
                wire_ids,
            });
        }
    }

    // Merge global nets with matching names across sheets
    let mut merged: HashMap<String, SchNet> = HashMap::new();
    for net in all_nets {
        if !net.auto {
            let key = net.name.clone();
            if let Some(existing) = merged.get_mut(&key) {
                existing.members.extend(net.members);
                existing.wire_ids.extend(net.wire_ids);
                existing.aliases.extend(net.aliases);
                existing.aliases.sort();
                existing.aliases.dedup();
                continue;
            }
        }
        merged.insert(net.id.clone(), net);
    }

    schematic.nets = merged.into_values().collect();
}
