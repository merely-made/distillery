// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! A model's own chat template, rendered as transformers renders it
//! (decision-driver D8).
//!
//! Opt-in per model; the inert `PromptTemplate` stays the default. A chat
//! template is a Jinja program from the model's repository. It runs in
//! minijinja with no loader, no filesystem, network or clock, and a fuel
//! limit. Its source bytes are what a model session binds by hash
//! (`BoundModelSession::prepare`), exactly as for an inert template.
use super::InferError;
use minijinja::{Environment, Error, ErrorKind, Value};
use serde::{Deserialize, Serialize};

/// Instruction budget for one render: far above any published template's
/// need for a bounded conversation, far below a runaway loop.
const FUEL: u64 = 2_000_000;

/// One turn, in the role/content shape Hugging Face templates read. Roles
/// are the template's own vocabulary (`system`, `user`, `assistant`, `tool`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

/// A chat template and the special tokens it may name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatTemplate {
    source: String,
    bos: String,
    eos: String,
}

fn invalid(detail: impl std::fmt::Display) -> InferError {
    InferError::InvalidConfig(format!("chat template: {detail}"))
}

/// Special tokens appear as plain strings or as `{"content": ...}` objects.
fn token(value: &serde_json::Value) -> String {
    value
        .as_str()
        .or_else(|| value["content"].as_str())
        .unwrap_or_default()
        .to_owned()
}

impl ChatTemplate {
    /// From template source and the special tokens it may reference.
    ///
    /// # Errors
    /// When the source is empty or does not parse.
    pub fn from_source(source: &str, bos: &str, eos: &str) -> Result<Self, InferError> {
        if source.trim().is_empty() {
            return Err(invalid("empty source"));
        }
        let template = Self {
            source: source.into(),
            bos: bos.into(),
            eos: eos.into(),
        };
        template
            .environment()
            .template_from_str(&template.source)
            .map_err(invalid)?;
        Ok(template)
    }

    /// From a model's `tokenizer_config.json`.
    ///
    /// # Errors
    /// When the file does not parse or carries no single chat template.
    pub fn from_tokenizer_config(bytes: &[u8]) -> Result<Self, InferError> {
        let config: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid)?;
        let source = config["chat_template"]
            .as_str()
            .ok_or_else(|| invalid("tokenizer config has no single chat_template string"))?;
        Self::from_source(source, &token(&config["bos_token"]), &token(&config["eos_token"]))
    }

    /// The exact bytes a model session binds by hash.
    pub fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    /// The end-of-turn token the template's model emits.
    pub fn eos(&self) -> &str {
        &self.eos
    }

    fn environment(&self) -> Environment<'static> {
        let mut environment = Environment::new();
        environment.set_fuel(Some(FUEL));
        // Templates call Python string methods (`startswith`, `split`, ...).
        environment.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
        environment.add_function("raise_exception", |message: String| -> Result<Value, Error> {
            Err(Error::new(ErrorKind::InvalidOperation, message))
        });
        environment
    }

    /// Render `messages` and open the assistant's turn. `variables` is a JSON
    /// object of template switches (`{"enable_thinking": false}`); it cannot
    /// replace the names the renderer sets.
    ///
    /// # Errors
    /// When `variables` is not an object or names a reserved variable, or the
    /// template refuses the conversation or exhausts its budget.
    pub fn render(
        &self,
        messages: &[ChatMessage],
        variables: &serde_json::Value,
    ) -> Result<String, InferError> {
        let refuse = |detail: String| InferError::InvalidRequest(format!("chat template: {detail}"));
        let mut context = match variables {
            serde_json::Value::Null => serde_json::Map::new(),
            serde_json::Value::Object(map) => map.clone(),
            _ => return Err(refuse("variables must be an object".into())),
        };
        for reserved in ["messages", "add_generation_prompt", "bos_token", "eos_token"] {
            if context.contains_key(reserved) {
                return Err(refuse(format!("variable {reserved:?} is set by the renderer")));
            }
        }
        context.insert("messages".into(), serde_json::json!(messages));
        context.insert("add_generation_prompt".into(), true.into());
        context.insert("bos_token".into(), self.bos.clone().into());
        context.insert("eos_token".into(), self.eos.clone().into());
        let environment = self.environment();
        environment
            .template_from_str(&self.source)
            .and_then(|template| template.render(Value::from(minijinja::value::Serde(&context))))
            .map_err(|error| refuse(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// ChatML in the shape of Qwen's template: a default system turn, Python
    /// string methods, a thinking switch and a refusal.
    const CHATML: &str = "{%- if messages[0].role != 'system' %}{{- '<|im_start|>system\\nBe brief.<|im_end|>\\n' }}{%- endif %}\
        {%- for message in messages %}\
        {%- if message.role not in ['system', 'user', 'assistant', 'tool'] %}{{- raise_exception('unknown role ' + message.role) }}{%- endif %}\
        {%- set content = message.content.strip() if message.content.startswith(' ') else message.content %}\
        {{- '<|im_start|>' + message.role + '\\n' + content + eos_token + '\\n' }}\
        {%- endfor %}\
        {%- if add_generation_prompt %}{{- '<|im_start|>assistant\\n' }}\
        {%- if enable_thinking is defined and enable_thinking is false %}{{- '<think>\\n\\n</think>\\n\\n' }}{%- endif %}{%- endif %}";

    fn chatml() -> ChatTemplate {
        ChatTemplate::from_source(CHATML, "", "<|im_end|>").unwrap()
    }

    #[test]
    fn a_conversation_renders_with_switches_and_python_methods() {
        let messages = [
            ChatMessage::new("user", " Which passages?"),
            ChatMessage::new("assistant", "{\"op\":\"brief\",\"order\":[0,2]}"),
            ChatMessage::new("tool", "accepted"),
        ];
        let open = "<|im_start|>system\nBe brief.<|im_end|>\n<|im_start|>user\nWhich passages?<|im_end|>\n\
                    <|im_start|>assistant\n{\"op\":\"brief\",\"order\":[0,2]}<|im_end|>\n\
                    <|im_start|>tool\naccepted<|im_end|>\n<|im_start|>assistant\n";
        let template = chatml();
        assert_eq!(template.render(&messages, &json!(null)).unwrap(), open);
        assert_eq!(
            template.render(&messages, &json!({"enable_thinking": false})).unwrap(),
            format!("{open}<think>\n\n</think>\n\n")
        );
        let own = [ChatMessage::new("system", "Cite."), ChatMessage::new("user", "Go")];
        assert!(template.render(&own, &json!({})).unwrap().starts_with("<|im_start|>system\nCite.<|im_end|>\n"));
        assert_eq!(template.source_bytes(), CHATML.as_bytes());
        assert_eq!(template.eos(), "<|im_end|>");
    }

    #[test]
    fn content_is_data_and_the_template_cannot_be_steered_or_run_away() {
        let template = chatml();
        // Template syntax inside content is text, never evaluated.
        let hostile = [ChatMessage::new("user", "{{ 7 * 7 }}{% for x in y %}")];
        assert!(template.render(&hostile, &json!({})).unwrap().contains("{{ 7 * 7 }}{% for x in y %}"));
        for (messages, variables, word) in [
            (vec![ChatMessage::new("wizard", "hi")], json!({}), "unknown role wizard"),
            (vec![ChatMessage::new("user", "hi")], json!({"messages": []}), "set by the renderer"),
            (vec![ChatMessage::new("user", "hi")], json!({"eos_token": "x"}), "set by the renderer"),
            (vec![ChatMessage::new("user", "hi")], json!([1]), "must be an object"),
            (vec![], json!({}), "chat template"),
        ] {
            let error = template.render(&messages, &variables).unwrap_err().to_string();
            assert!(error.contains(word), "{error}");
        }
        let endless = ChatTemplate::from_source(
            "{% for a in range(100000) %}{% for b in range(100000) %}x{% endfor %}{% endfor %}",
            "",
            "",
        )
        .unwrap();
        assert!(endless.render(&[], &json!({})).is_err());
    }

    #[test]
    fn sources_and_tokenizer_configs_are_checked_at_load() {
        assert!(ChatTemplate::from_source("  ", "", "").is_err());
        assert!(ChatTemplate::from_source("{% if %}", "", "").is_err());
        let config = json!({"chat_template": CHATML, "bos_token": null,
                            "eos_token": {"content": "<|im_end|>"}});
        let loaded = ChatTemplate::from_tokenizer_config(config.to_string().as_bytes()).unwrap();
        assert_eq!(loaded, chatml());
        for bad in [json!({}), json!({"chat_template": [{"name": "default"}]})] {
            assert!(ChatTemplate::from_tokenizer_config(bad.to_string().as_bytes()).is_err());
        }
    }

    /// The real Qwen3 template against the rendering the decision tests used.
    #[test]
    #[ignore = "needs a Qwen3 tokenizer_config.json (ESP_QWEN3_DIR)"]
    fn qwen3s_own_template_renders_its_chatml() {
        let dir = std::path::PathBuf::from(std::env::var_os("ESP_QWEN3_DIR").expect("ESP_QWEN3_DIR"));
        let template =
            ChatTemplate::from_tokenizer_config(&std::fs::read(dir.join("tokenizer_config.json")).unwrap())
                .unwrap();
        let messages = [ChatMessage::new("system", "Cite."), ChatMessage::new("user", "Which?")];
        assert_eq!(
            template.render(&messages, &json!({"enable_thinking": false})).unwrap(),
            "<|im_start|>system\nCite.<|im_end|>\n<|im_start|>user\nWhich?<|im_end|>\n\
             <|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
        assert_eq!(template.eos(), "<|im_end|>");
    }
}
