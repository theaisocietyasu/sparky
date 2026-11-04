//! Compaction: what it replaces, what it keeps, and what happens when it fails.

use std::sync::Arc;
use std::time::Duration;

use crate::core::tests::support::{Row, Scripted, history_of, push_row, text};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::conversation::compaction::Compactor;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Stored;
use crate::core::types::conversation::message::{Message, Role, ToolCall};
use crate::core::types::model::ModelError;
use crate::core::types::store::StoreError;
use crate::runtime::harness::agent::task::{Task, TaskConfig};
use crate::runtime::harness::compact::{ChatCompactor, transcript};

fn ctx() -> RequestContext {
    RequestContext::new("g", "u", Duration::from_secs(5))
}

fn compactor(
    replies: Vec<Result<crate::core::types::model::ModelResponse, ModelError>>,
) -> ChatCompactor {
    ChatCompactor::new(Task::new(
        Arc::new(Scripted::new(replies)),
        "compaction",
        "compact this",
        TaskConfig::default(),
    ))
}

#[test]
fn the_transcript_names_who_said_what_and_keeps_tool_calls() {
    let turns = vec![
        Message::user("when does hayden close"),
        Message::assistant_tool_calls(
            "",
            vec![ToolCall {
                id: "1".into(),
                name: "search_library_hours".into(),
                arguments: serde_json::json!({"query": "hayden hours"}),
            }],
        ),
        Message::tool_result("1", "search_library_hours", "2am on weekdays"),
        Message::assistant("Hayden closes at 2am on weekdays."),
    ];
    let out = transcript(&turns);
    assert!(out.contains("User: when does hayden close"), "{out}");
    assert!(out.contains("Sparky called search_library_hours"), "{out}");
    assert!(out.contains("Tool result: 2am on weekdays"), "{out}");
    assert!(out.contains("Sparky: Hayden closes"), "{out}");
}

#[tokio::test]
async fn a_compacted_turn_carries_its_own_role() {
    let c = compactor(vec![Ok(text(
        "The student asked about Hayden hours. It closes at 2am.",
    ))]);
    let turns = vec![Message::user("when does hayden close")];
    let Ok(summary) = c.compact(&ctx(), &turns).await else {
        unreachable!("the model answered")
    };
    // The summary has the summary role and no tool calls.
    assert_eq!(summary.role, Role::Summary);
    assert!(summary.content.contains("Hayden"));
    assert_eq!(summary.tool_calls.len(), 0);
}

#[tokio::test]
async fn nothing_to_compact_is_an_error_rather_than_an_empty_summary() {
    let c = compactor(vec![Ok(text("unused"))]);
    let blank = vec![Message::assistant("")];
    assert!(c.compact(&ctx(), &blank).await.is_err());
}

#[tokio::test]
async fn a_model_that_answers_with_nothing_is_an_error() {
    let c = compactor(vec![Ok(text("   "))]);
    let turns = vec![Message::user("something")];
    assert!(c.compact(&ctx(), &turns).await.is_err());
}

/// A conversation store preloaded with history, keeping positions and summary coverage.
#[derive(Default)]
struct Loaded {
    rows: std::sync::Mutex<Vec<Row>>,
}

impl Loaded {
    fn seed(&self, messages: impl IntoIterator<Item = Message>) {
        if let Ok(mut rows) = self.rows.lock() {
            for m in messages {
                push_row(&mut rows, m, None);
            }
        }
    }

    fn messages(&self) -> Vec<Message> {
        self.rows
            .lock()
            .map(|rows| rows.iter().map(|r| r.message.clone()).collect())
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl ConversationStore for Loaded {
    async fn ensure(&self, _ctx: &RequestContext, _channel_id: &str) -> Result<(), StoreError> {
        Ok(())
    }

    async fn owns(&self, _ctx: &RequestContext) -> Result<bool, StoreError> {
        Ok(true)
    }

    async fn latest(
        &self,
        _ctx: &RequestContext,
        _channel_id: &str,
    ) -> Result<Option<uuid::Uuid>, StoreError> {
        Ok(None)
    }

    async fn end(&self, _ctx: &RequestContext, _channel_id: &str) -> Result<u64, StoreError> {
        Ok(0)
    }

    async fn load(&self, _ctx: &RequestContext, limit: usize) -> Result<Vec<Stored>, StoreError> {
        Ok(self
            .rows
            .lock()
            .map(|rows| history_of(&rows, limit))
            .unwrap_or_default())
    }

    async fn append(&self, _ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError> {
        if let Ok(mut rows) = self.rows.lock() {
            for turn in turns {
                push_row(&mut rows, turn.clone(), None);
            }
        }
        Ok(())
    }

    async fn append_summary(
        &self,
        _ctx: &RequestContext,
        summary: &Message,
        covers: i64,
    ) -> Result<(), StoreError> {
        if let Ok(mut rows) = self.rows.lock() {
            push_row(&mut rows, summary.clone(), Some(covers));
        }
        Ok(())
    }
}

#[tokio::test]
async fn history_over_budget_is_replaced_by_one_turn_that_is_kept() {
    use crate::core::tests::support::MemorySink;
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::agent::assemble::Budget;
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let store = Arc::new(Loaded::default());
    store.seed((0..40).flat_map(|i| {
        [
            Message::user(format!("question {i} about ASU library hours and events")),
            Message::assistant(format!("answer {i} with enough text to cost tokens")),
        ]
    }));
    let before = store.messages().len();

    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![Ok(text("done"))])),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::new()),
        conversations: Some(store.clone()),
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        compactor: Some(Arc::new(compactor(vec![Ok(text(
            "Forty exchanges about hours.",
        ))]))),
        guardrail: None,
    };
    let cfg = AgentConfig {
        budget: Budget {
            history: 200,
            ..Budget::default()
        },
        history_keep: 80,
        ..AgentConfig::default()
    };
    let agent = Agent::new(deps, cfg, "sys");
    let Ok(answer) = agent.run(&ctx(), "and now?").await else {
        unreachable!("the run completed")
    };
    assert_eq!(answer.text, "done");

    let after = store.messages();
    let summaries: Vec<&Message> = after.iter().filter(|m| m.role == Role::Summary).collect();
    assert_eq!(summaries.len(), 1, "one compacted turn was stored");
    assert!(summaries[0].content.contains("Forty exchanges"));
    // The turns it replaced are still there. The summary stands in for them at assembly time.
    assert!(after.len() > before, "nothing was deleted");
}

#[tokio::test]
async fn a_failed_compaction_leaves_the_run_working() {
    use crate::core::tests::support::MemorySink;
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::agent::assemble::Budget;
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let store = Arc::new(Loaded::default());
    store.seed(
        (0..40).map(|i| Message::user(format!("question {i} padded out to spend the budget"))),
    );
    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![Ok(text("answered anyway"))])),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::new()),
        conversations: Some(store.clone()),
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        // The compactor has no scripted reply, so its model call fails.
        compactor: Some(Arc::new(compactor(Vec::new()))),
        guardrail: None,
    };
    let cfg = AgentConfig {
        budget: Budget {
            history: 200,
            ..Budget::default()
        },
        ..AgentConfig::default()
    };
    let agent = Agent::new(deps, cfg, "sys");
    let Ok(answer) = agent.run(&ctx(), "and now?").await else {
        unreachable!("a failed compaction does not fail the request")
    };
    assert_eq!(answer.text, "answered anyway");
    let stored = store.messages();
    assert!(
        !stored.iter().any(|m| m.role == Role::Summary),
        "no summary is stored when compaction fails"
    );
}

fn agent_over(
    store: Arc<Loaded>,
    summarizer: Scripted,
    chat: Scripted,
    history: usize,
    keep: usize,
) -> crate::runtime::harness::agent::Agent {
    use crate::core::tests::support::MemorySink;
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::agent::assemble::Budget;
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let deps = AgentDeps {
        model: Arc::new(chat),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::new()),
        conversations: Some(store),
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        compactor: Some(Arc::new(ChatCompactor::new(Task::new(
            Arc::new(summarizer),
            "compaction",
            "compact this",
            TaskConfig::default(),
        )))),
        guardrail: None,
    };
    let cfg = AgentConfig {
        budget: Budget {
            history,
            ..Budget::default()
        },
        history_keep: keep,
        ..AgentConfig::default()
    };
    Agent::new(deps, cfg, "sys")
}

fn contents(
    sent: &std::sync::Mutex<Vec<crate::core::types::model::ModelRequest>>,
    at: usize,
) -> Vec<Message> {
    sent.lock()
        .map(|r| r.get(at).map(|q| q.messages.clone()).unwrap_or_default())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_compaction_leaves_room_so_the_next_request_reads_the_summary_and_compacts_nothing() {
    let pad = "x".repeat(80);
    let store = Arc::new(Loaded::default());
    store.seed([
        Message::user(format!("first question {pad}")),
        Message::assistant(format!("first answer {pad}")),
        Message::user(format!("second question {pad}")),
        Message::assistant(format!("second answer {pad}")),
        Message::user(format!("KEPT-QUESTION {pad}")),
        Message::assistant(format!("KEPT-ANSWER {pad}")),
    ]);
    let summarizer = Scripted::new(vec![Ok(text("The user asked two questions."))]);
    let transcripts = summarizer.sent();
    let chat = Scripted::new(vec![
        Ok(text(&format!("fourth answer {pad}"))),
        Ok(text("fifth answer")),
    ]);
    let prompts = chat.sent();
    let agent = agent_over(store, summarizer, chat, 150, 60);

    assert!(agent.run(&ctx(), "turn four").await.is_ok());
    assert!(agent.run(&ctx(), "turn five").await.is_ok());

    let compactions = transcripts.lock().map(|r| r.len()).unwrap_or_default();
    assert_eq!(
        compactions, 1,
        "the second request fits and compacts nothing"
    );
    let first: String = contents(&prompts, 0)
        .iter()
        .map(|m| m.content.clone())
        .collect();
    assert!(
        first.contains("KEPT-ANSWER"),
        "the kept turns reach the prompt"
    );
    assert!(
        !first.contains("first question"),
        "the replaced turns do not"
    );
    let second = contents(&prompts, 1);
    assert!(
        second
            .iter()
            .any(|m| m.role == Role::Summary && m.content.contains("two questions")),
        "the stored summary reaches the next prompt: {second:?}"
    );
    assert!(second.iter().any(|m| m.content.contains("KEPT-QUESTION")));
}

#[tokio::test]
async fn the_kept_turns_start_on_a_question_never_on_a_tool_result() {
    let pad = "x".repeat(80);
    let store = Arc::new(Loaded::default());
    store.seed([
        Message::user(format!("first question {pad}")),
        Message::assistant_tool_calls(
            "",
            vec![ToolCall {
                id: "c1".into(),
                name: "search_knowledge".into(),
                arguments: serde_json::json!({"query": "hours"}),
            }],
        ),
        Message::tool_result("c1", "search_knowledge", format!("TOOL-RESULT {pad}")),
        Message::assistant(format!("first answer {pad}")),
        Message::user(format!("second question {pad}")),
        Message::assistant(format!("second answer {pad}")),
    ]);
    let summarizer = Scripted::new(vec![Ok(text("Asked about hours."))]);
    let transcripts = summarizer.sent();
    let chat = Scripted::new(vec![Ok(text("ok"))]);
    let prompts = chat.sent();
    // The keep budget reaches back to the tool result, so the tail is moved up to a question.
    let agent = agent_over(store, summarizer, chat, 120, 110);

    assert!(agent.run(&ctx(), "next").await.is_ok());

    let replaced: String = transcripts
        .lock()
        .map(|r| {
            r.iter()
                .flat_map(|q| q.messages.iter().map(|m| m.content.clone()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        replaced.contains("TOOL-RESULT"),
        "the tool result is summarized"
    );
    let prompt = contents(&prompts, 0);
    assert!(
        !prompt.iter().any(|m| m.role == Role::Tool),
        "no orphaned tool result reaches the prompt: {prompt:?}"
    );
    assert!(prompt.iter().any(|m| m.content.contains("second question")));
}
