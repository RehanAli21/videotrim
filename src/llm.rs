use crate::EditPlan;
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        parameters::{FormatType, JsonStructure},
    },
    models::ModelOptions,
    Ollama,
};

pub async fn get_plan_from_model(
    model: &str,
    user_instructions: &str,
    transcript: &str,
) -> Result<EditPlan, String> {
    let ollama = Ollama::default();

    let prompt = format!(
        "You are a video editor. Below is a transcript with timestamps in seconds.\n\
        Identify segments to CUT: filler words (um, uh, like), long silences, \
        repeated sentences, false starts, and off-topic rambling.\n\n\
        Additional instructions from the user:\n{}\n\n\
        Return a JSON object with an \"edits\" array. Each edit has:\n\
        - \"start\": start time in seconds (number)\n\
        - \"end\": end time in seconds (number)\n\
        - \"reason\": why it should be cut (string)\n\n\
        - \"text\": Whatever text in the segment (string)\n\n\
        Example: {{\"edits\": [{{\"start\": 2.5, \"end\": 4.0, \"reason\": \"filler word um\", \"text\":\"segment text\"}}]}}\n\n\
        Transcript:\n{}",
        user_instructions,
        transcript
    );

    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<EditPlan>()));

    let options = ModelOptions::default().temperature(0.0);
    let request = GenerationRequest::new(model.to_string(), prompt)
        .format(format)
        .options(options);

    let res = ollama.generate(request).await;

    let response = match res {
        Ok(r) => r.response,
        Err(err) => {
            return Err(format!(
                "Err in getting response from ollama generate. err => {}",
                err
            ))
        }
    };

    let plan: EditPlan = match serde_json::from_str(&response) {
        Ok(json) => json,
        Err(err) => {
            return Err(format!(
                "Err on converting ollama response to json. err => {}",
                err
            ))
        }
    };

    Ok(plan)
}
