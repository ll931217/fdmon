use crate::proc::ProcessInfo;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub process: ProcessInfo,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub is_ancestor_only: bool,
}

/// Builds a process tree showing only processes with FDs > 0 and their ancestor chains
pub fn build_tree(processes: &[ProcessInfo]) -> Vec<TreeNode> {
    // Build parent-child mapping
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let process_map: HashMap<u32, ProcessInfo> = processes
        .iter()
        .map(|p| (p.pid, p.clone()))
        .collect();

    for process in processes {
        children_map
            .entry(process.ppid)
            .or_default()
            .push(process.pid);
    }

    // Mark processes with FDs > 0 as interesting
    let mut interesting_pids = HashSet::new();
    for process in processes {
        if process.fd_count > 0 {
            interesting_pids.insert(process.pid);
        }
    }

    // Walk ancestors of interesting processes to root
    let mut marked_pids = interesting_pids.clone();
    for &pid in &interesting_pids {
        mark_ancestors(pid, &process_map, &mut marked_pids);
    }

    // Find root processes (ppid == 0 or ppid not in process list)
    let mut roots = Vec::new();
    for process in processes {
        if process.ppid == 0 || !process_map.contains_key(&process.ppid) {
            if marked_pids.contains(&process.pid) {
                roots.push(process.pid);
            }
        }
    }

    // DFS to build flattened tree
    let mut tree_nodes = Vec::new();
    for root in roots {
        build_tree_recursive(
            root,
            0,
            &process_map,
            &children_map,
            &interesting_pids,
            &marked_pids,
            &mut tree_nodes,
        );
    }

    tree_nodes
}

/// Marks all ancestors of a process
fn mark_ancestors(
    pid: u32,
    process_map: &HashMap<u32, ProcessInfo>,
    marked: &mut HashSet<u32>,
) {
    if let Some(process) = process_map.get(&pid) {
        let ppid = process.ppid;
        if ppid != 0 && process_map.contains_key(&ppid) {
            marked.insert(ppid);
            mark_ancestors(ppid, process_map, marked);
        }
    }
}

/// Recursively builds tree nodes via DFS
fn build_tree_recursive(
    pid: u32,
    depth: usize,
    process_map: &HashMap<u32, ProcessInfo>,
    children_map: &HashMap<u32, Vec<u32>>,
    interesting_pids: &HashSet<u32>,
    marked_pids: &HashSet<u32>,
    tree_nodes: &mut Vec<TreeNode>,
) {
    let Some(process) = process_map.get(&pid) else {
        return;
    };

    // Get marked children
    let marked_children: Vec<u32> = children_map
        .get(&pid)
        .map(|children| {
            children
                .iter()
                .copied()
                .filter(|child_pid| marked_pids.contains(child_pid))
                .collect()
        })
        .unwrap_or_default();

    let has_children = !marked_children.is_empty();
    let is_ancestor_only = !interesting_pids.contains(&pid);

    tree_nodes.push(TreeNode {
        process: process.clone(),
        depth,
        expanded: true, // Default to expanded
        has_children,
        is_ancestor_only,
    });

    // Recurse into children
    for child_pid in marked_children {
        build_tree_recursive(
            child_pid,
            depth + 1,
            process_map,
            children_map,
            interesting_pids,
            marked_pids,
            tree_nodes,
        );
    }
}

/// Toggles expansion state of a tree node
pub fn toggle_expand(tree_nodes: &[TreeNode], index: usize) -> Vec<TreeNode> {
    if index >= tree_nodes.len() {
        return tree_nodes.to_vec();
    }

    let mut result = tree_nodes.to_vec();
    result[index].expanded = !result[index].expanded;

    // If collapsing, hide children
    if !result[index].expanded {
        let target_depth = result[index].depth;
        let i = index + 1;

        while i < result.len() && result[i].depth > target_depth {
            result.remove(i);
        }
    }

    result
}
