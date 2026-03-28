use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use zt_core::note::NoteId;

/// Directed graph of inter-note links.
pub struct LinkGraph {
    graph: DiGraph<NoteId, ()>,
    node_map: HashMap<NoteId, NodeIndex>,
}

impl LinkGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// Ensure a note exists in the graph, returning its node index.
    pub fn add_note(&mut self, id: NoteId) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(&id) {
            return idx;
        }
        let idx = self.graph.add_node(id.clone());
        self.node_map.insert(id, idx);
        idx
    }

    /// Add a directed link from source to target.
    pub fn add_link(&mut self, source: &NoteId, target: &NoteId) {
        let src = self.add_note(source.clone());
        let tgt = self.add_note(target.clone());
        // Avoid duplicate edges
        if !self.graph.contains_edge(src, tgt) {
            self.graph.add_edge(src, tgt, ());
        }
    }

    /// Remove all outgoing edges from a note (used before re-indexing a file).
    pub fn clear_outgoing(&mut self, source: &NoteId) {
        let Some(&src_idx) = self.node_map.get(source) else {
            return;
        };
        let edges_to_remove: Vec<_> = self
            .graph
            .edges(src_idx)
            .map(|e| e.id())
            .collect();
        for edge in edges_to_remove {
            self.graph.remove_edge(edge);
        }
    }

    /// Get all notes that this note links to (outgoing).
    pub fn outgoing(&self, note: &NoteId) -> Vec<&NoteId> {
        let Some(&idx) = self.node_map.get(note) else {
            return vec![];
        };
        self.graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|n| &self.graph[n])
            .collect()
    }

    /// Get all notes that link to this note (backlinks / incoming).
    pub fn backlinks(&self, note: &NoteId) -> Vec<&NoteId> {
        let Some(&idx) = self.node_map.get(note) else {
            return vec![];
        };
        self.graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| &self.graph[n])
            .collect()
    }

    /// Get all notes in the graph.
    pub fn all_notes(&self) -> Vec<&NoteId> {
        self.graph.node_weights().collect()
    }

    /// Get all edges as (source, target) pairs.
    pub fn all_edges(&self) -> Vec<(&NoteId, &NoteId)> {
        self.graph
            .edge_indices()
            .filter_map(|e| {
                let (src, tgt) = self.graph.edge_endpoints(e)?;
                Some((&self.graph[src], &self.graph[tgt]))
            })
            .collect()
    }

    /// Total number of notes.
    pub fn note_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total number of links.
    pub fn link_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl Default for LinkGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backlinks_work() {
        let mut g = LinkGraph::new();
        let a = NoteId("a".into());
        let b = NoteId("b".into());
        let c = NoteId("c".into());

        g.add_link(&a, &b);
        g.add_link(&c, &b);

        let backlinks = g.backlinks(&b);
        assert_eq!(backlinks.len(), 2);
        assert!(backlinks.contains(&&a));
        assert!(backlinks.contains(&&c));
    }

    #[test]
    fn clear_outgoing_works() {
        let mut g = LinkGraph::new();
        let a = NoteId("a".into());
        let b = NoteId("b".into());
        let c = NoteId("c".into());

        g.add_link(&a, &b);
        g.add_link(&a, &c);
        assert_eq!(g.outgoing(&a).len(), 2);

        g.clear_outgoing(&a);
        assert_eq!(g.outgoing(&a).len(), 0);
        // b and c still exist as nodes
        assert_eq!(g.note_count(), 3);
    }
}
