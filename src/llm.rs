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
    user_instructions: &str,
    transcript: &str,
) -> Result<EditPlan, String> {
    let ollama = Ollama::default();

    let model = "qwen2.5:7b".to_string();

    let prompt = format!(
        "You are a video editor. Below is a transcript with timestamps in seconds.\n\
        Identify segments to CUT: filler words (um, uh, like), long silences, \
        repeated sentences, false starts, and off-topic rambling.\n\n\
        Additional instructions from the user:\n{}\n\n\
        Return a JSON object with an \"edits\" array. Each edit has:\n\
        - \"cut_from\": start time in seconds (number)\n\
        - \"cut_to\": end time in seconds (number)\n\
        - \"reason\": why it should be cut (string)\n\n\
        Example: {{\"edits\": [{{\"cut_from\": 2.5, \"cut_to\": 4.0, \"reason\": \"filler word um\"}}]}}\n\n\
        Transcript:\n{}",
        user_instructions,
        transcript
    );

    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<EditPlan>()));

    let options = ModelOptions::default().temperature(0.0);
    let request = GenerationRequest::new(model, prompt)
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
