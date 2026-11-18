//! Iterative Tarjan strongly-connected components for cycle detection.

/// Returns SCCs containing more than one node (real cycles). `nodes` are
/// indices; `edges` are directed (from, to) index pairs.
pub fn strongly_connected_cycles(node_count: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    for (from, to) in edges {
        if *from < node_count && *to < node_count {
            adjacency[*from].push(*to);
        }
    }

    #[derive(Clone, Copy)]
    struct Frame {
        node: usize,
        child_index: usize,
    }

    let mut index: Vec<Option<u32>> = vec![None; node_count];
    let mut lowlink = vec![0u32; node_count];
    let mut on_stack = vec![false; node_count];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter: u32 = 0;
    let mut call_stack: Vec<Frame> = Vec::new();
    let mut cycles: Vec<Vec<usize>> = Vec::new();

    for start in 0..node_count {
        if index[start].is_some() {
            continue;
        }
        call_stack.push(Frame {
            node: start,
            child_index: 0,
        });
        while let Some(frame) = call_stack.last_mut() {
            let node = frame.node;
            if frame.child_index == 0 {
                index[node] = Some(counter);
                lowlink[node] = counter;
                counter += 1;
                stack.push(node);
                on_stack[node] = true;
            }
            if frame.child_index < adjacency[node].len() {
                let next = adjacency[node][frame.child_index];
                frame.child_index += 1;
                if index[next].is_none() {
                    call_stack.push(Frame {
                        node: next,
                        child_index: 0,
                    });
                } else if on_stack[next] {
                    lowlink[node] = lowlink[node].min(index[next].unwrap());
                }
            } else {
                call_stack.pop();
                if let Some(parent) = call_stack.last_mut() {
                    lowlink[parent.node] = lowlink[parent.node].min(lowlink[node]);
                }
                if lowlink[node] == index[node].unwrap() {
                    let mut component = Vec::new();
                    while let Some(top) = stack.pop() {
                        on_stack[top] = false;
                        component.push(top);
                        if top == node {
                            break;
                        }
                    }
                    if component.len() > 1 {
                        cycles.push(component);
                    }
                }
            }
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_three_node_cycle() {
        // a→b→c→a plus a dead branch
        let edges = vec![(0, 1), (1, 2), (2, 0), (0, 3)];
        let cycles = strongly_connected_cycles(4, &edges);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
    }

    #[test]
    fn acyclic_graph_has_no_cycles() {
        let edges = vec![(0, 1), (1, 2), (0, 2)];
        assert!(strongly_connected_cycles(3, &edges).is_empty());
    }
}
