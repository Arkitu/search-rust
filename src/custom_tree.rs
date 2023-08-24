pub struct Node<P: Ord, T> {
    position: P,
    value: T,
    lower: Option<Box<Node<P, T>>>,
    higher: Option<Box<Node<P, T>>>
}
impl<P: Ord, T> Node<P, T> {
    pub fn new(position: P, value: T) -> Self {
        Node {
            position,
            value,
            lower: None,
            higher: None
        }
    }
    pub fn nearest_node(&self, position: &P) -> &Node<P, T> {
        if position < &self.position {
            match self.lower {
                Some(ref lower) => {
                    lower.nearest_node(position)
                },
                None => {
                    self
                }
            }
        } else {
            match self.higher {
                Some(ref higher) => {
                    higher.nearest_node(position)
                },
                None => {
                    self
                }
            }
        }
    }
    pub fn nearest_node_mut(&mut self, position: &P) -> &mut Node<P, T> {
        if position < &self.position {
            match self.lower {
                Some(ref mut lower) => {
                    lower.nearest_node_mut(position)
                },
                None => {
                    self
                }
            }
        } else {
            match self.higher {
                Some(ref mut higher) => {
                    higher.nearest_node_mut(position)
                },
                None => {
                    self
                }
            }
        }
    }
    pub fn insert(&mut self, position: P, value: T) {
        let near = self.nearest_node_mut(&position);
        if position < near.position {
            near.lower = Some(Box::new(Node::new(position, value)));
        } else {
            near.higher = Some(Box::new(Node::new(position, value)));
        }
    }
    pub fn get(&self, position: &P) -> Option<&T> {
        let near = self.nearest_node(position);
        if &near.position == position {
            Some(&near.value)
        } else {
            None
        }
    }
    pub fn get_mut(&mut self, position: &P) -> Option<&mut T> {
        let near = self.nearest_node_mut(position);
        if &near.position == position {
            Some(&mut near.value)
        } else {
            None
        }
    }
    pub fn nearest(&self, position: &P) -> Option<&T> {
        let near = self.nearest_node(position);
        Some(&near.value)
    }
    pub fn nearest_n(&self, position: &P, n: usize) -> Vec<&T> {
        let mut result = Vec::new();
        let mut current = self.nearest_node(position);
        let mut i = 0;
        while i < n {
            result.push(&current.value);
            if let Some(ref higher) = current.higher {
                current = higher;
            } else {
                break;
            }
            i += 1;
        }
        current = self.nearest_node(position);
        i = 0;
        while i < n {
            result.push(&current.value);
            if let Some(ref lower) = current.lower {
                current = lower;
            } else {
                break;
            }
            i += 1;
        }
        result
    }
}