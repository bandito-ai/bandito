use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

pub fn script(sdk: &str) -> Result<()> {
    match sdk {
        "python" => write_template("bandito_example.py", PYTHON_TEMPLATE),
        "js" | "javascript" | "typescript" | "ts" => {
            write_template("bandito_example.ts", JS_TEMPLATE)
        }
        _ => bail!("Unknown SDK: {}. Use --sdk python or --sdk js", sdk),
    }
}

pub fn bandit(name: &str) -> Result<()> {
    let filename = format!("{}.json", name);
    let content = serde_json::json!({
        "name": name,
        "description": "",
        "type": "online",
        "cost_importance": 2,
        "latency_importance": 2,
        "optimization_mode": "base",
        "arms": [
            {
                "model": "gpt-4o",
                "provider": "openai",
                "prompt": "You are a helpful assistant."
            },
            {
                "model": "claude-sonnet-4-20250514",
                "provider": "anthropic",
                "prompt": "You are a helpful assistant."
            }
        ]
    });
    let json = serde_json::to_string_pretty(&content)?;
    write_template(&filename, &json)
}

fn write_template(filename: &str, content: &str) -> Result<()> {
    let path = Path::new(filename);
    if path.exists() {
        bail!("{} already exists", filename);
    }
    fs::write(path, content)?;
    println!("Created {}", filename);
    Ok(())
}

pub const PYTHON_TEMPLATE: &str = r#""""Bandito SDK starter — edit the LLM call to match your setup."""

import bandito

# Connect to Bandito cloud (reads ~/.bandito/config.toml)
bandito.connect()

user_message = "What is the meaning of life?"

# Pull the best arm (<1ms, no network call)
result = bandito.pull("my-chatbot", query=user_message)

print(f"Selected: {result.model} / {result.provider}")
print(f"Prompt: {result.prompt}")

# --- Replace with your actual LLM call ---
# from openai import OpenAI
# client = OpenAI()
# response = client.chat.completions.create(
#     model=result.model,
#     messages=[
#         {"role": "system", "content": result.prompt},
#         {"role": "user", "content": user_message},
#     ],
# )
# text = response.choices[0].message.content
# usage = response.usage
response_text = "This is a placeholder response."
input_tokens = 25
output_tokens = 50
# ---

# Report the outcome
bandito.update(
    result,
    query_text=user_message,
    response=response_text,
    input_tokens=input_tokens,
    output_tokens=output_tokens,
)

bandito.close()
"#;

pub const JS_TEMPLATE: &str = r#"/**
 * Bandito SDK starter — edit the LLM call to match your setup.
 */

import { connect, pull, update, close } from "bandito";

// Connect to Bandito cloud (reads ~/.bandito/config.toml)
await connect();

const userMessage = "What is the meaning of life?";

// Pull the best arm (<1ms, no network call)
const result = pull("my-chatbot", { query: userMessage });

console.log(`Selected: ${result.model} / ${result.provider}`);
console.log(`Prompt: ${result.prompt}`);

// --- Replace with your actual LLM call ---
// import OpenAI from "openai";
// const client = new OpenAI();
// const response = await client.chat.completions.create({
//   model: result.model,
//   messages: [
//     { role: "system", content: result.prompt },
//     { role: "user", content: userMessage },
//   ],
// });
// const text = response.choices[0].message.content;
// const usage = response.usage;
const responseText = "This is a placeholder response.";
const inputTokens = 25;
const outputTokens = 50;
// ---

// Report the outcome
update(result, {
  queryText: userMessage,
  response: responseText,
  inputTokens,
  outputTokens,
});

await close();
"#;
