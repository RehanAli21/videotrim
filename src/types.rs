use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
pub struct EditCommand {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct EditPlan {
    pub reasoning: String,
    pub edits: Vec<EditCommand>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct TimelineSegment {
    pub index: usize,
    pub start: f64,
    pub end: f64,
}
