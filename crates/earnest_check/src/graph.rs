#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    pub label: String,
    pub group: Option<String>,
    pub start: bool,
    pub end: bool,
    pub stuck: bool,
    pub broken: Vec<String>,
    pub states: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub title: Option<String>,
    pub colors: Vec<(String, String)>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
