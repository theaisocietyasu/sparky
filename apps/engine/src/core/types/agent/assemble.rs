//! Budget, Sections, and Assembled are the inputs and output of context assembly.

use crate::core::types::conversation::message::Message;
use crate::core::types::memory::Memory;

/// Budgets for one assembled prompt, in estimated tokens. Default is implemented in core::config.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Whole prompt, everything included.
    pub total: usize,
    /// Cap on prior turns.
    pub history: usize,
    /// Cap on the memory section.
    pub memory: usize,
    /// Cap on the capabilities section.
    pub capabilities: usize,
    /// Cap on the quoted message a reply answers.
    pub reply: usize,
    /// Characters per token the estimator assumes. Never zero.
    pub chars_per_token: usize,
}

/// The text assembly writes around the sections. Every field is configurable.
#[derive(Debug, Clone, Copy)]
pub struct Templates<'a> {
    /// Line naming the user, with {user} and {roles}.
    pub role_line: &'a str,
    /// Line naming a user who holds no roles, with {user}.
    pub role_line_no_roles: &'a str,
    /// Heading above recalled memories.
    pub memory_header: &'a str,
    /// Heading above what the model may do.
    pub capabilities_header: &'a str,
    /// Line naming the current date, with {date}.
    pub date_line: &'a str,
    /// Heading above the message of yours a reply answers.
    pub reply_header: &'a str,
    /// Line closing a tool result cut to fit the prompt, with {chars}.
    pub result_cut_line: &'a str,
    /// What to remember and what never to store. Added to the system prompt in a private turn.
    pub memory_rules: &'a str,
    /// How to use memories to find events and clubs. Added to the system prompt in a private turn.
    pub discovery_rules: &'a str,
}

/// Default line naming the user, with {user} and {roles}.
pub const ROLE_LINE: &str = "The user is `{user}`. Roles: {roles}.";
/// Default line naming a user who holds no roles, with {user}.
pub const ROLE_LINE_NO_ROLES: &str = "The user is `{user}`. They hold no special roles.";
/// Default heading above recalled memories.
pub const MEMORY_HEADER: &str = "What you remember about this user:";
/// Default rules for the memory tools, added to the system prompt in a private turn.
pub const MEMORY_RULES: &str = "Memory. Call remember when the student states something about themselves that helps \
later: their major, year, campus, clubs, how they want answers, or a task with its due date. \
Only what they said, never a guess. Never store grades, GPA, health, disability, immigration \
status, finances, passwords or codes, or anything about another person. Call forget_memory \
with the id in brackets when they ask you to forget something.";
/// Default rule for finding events and clubs from memories, added in a private turn.
pub const DISCOVERY_RULES: &str = "Discovery. When the student asks for something to do, join or attend, take their major, interests, clubs and campus from what you remember, use them as the words of a search_live query on events or clubs, and say which memory you used.";
/// Default line naming the current date, with {date}.
pub const DATE_LINE: &str = "Today is {date}. Search results are labelled by day or date; read \
                             the label the question asks for, never the first value in a row.";
/// Default line naming a file the user attached and its text.
pub const UPLOAD_LINE: &str = "The user attached {name} ({kind}, {size}). Its text, {chars} characters, is at {text_path} in the sandbox workspace, session {session}; the file itself is at {path}. It begins:\n{preview}\n\nThe passages of it that hold the most words of the question:\n{matches}\n\nAnswer a question about this file from these passages, never from search results or memory. When they do not hold the answer, read more with run_sandbox in session {session}: rg -i -n -C 3 SUBJECT {text_path}, or sed -n START,ENDp {text_path} around a line number above.";
/// Default line naming a file no text could be pulled from.
pub const UPLOAD_RAW_LINE: &str = "The user attached {name} ({kind}, {size}). It is at {path} in the sandbox workspace, session {session}, and no text could be pulled from it automatically. Open it with run_sandbox in that session before you answer: file {path} says what it is, and python3 with pypdf, docx, openpyxl or pandas, or pdftoppm and tesseract for a scan, reads it.";
/// Default line naming a file the user attached that could not be opened, with {reason}.
pub const UPLOAD_FAILED_LINE: &str = "The user attached {name} ({kind}), but it could not be opened: {reason}. Say so if the question depends on it.";
/// Default question sent in place of an empty message that carries attachments.
pub const ATTACHMENTS_ONLY_INPUT: &str =
    "I sent the attached file without a question. Tell me what it is and what it says.";
/// Default line sent back when a tool failed and the sandbox was not tried.
pub const SANDBOX_RETRY_LINE: &str = "A tool you called this turn failed, and you have not \
                                      opened the page yourself. Do that now: call run_sandbox \
                                      and fetch the page the question is about, then answer from \
                                      what it prints. Tell the user you could not find something \
                                      only after that has been tried.";
/// Default line added to a call that is offered no tools.
pub const ANSWER_ONLY_LINE: &str = "You have no tools on this step. Answer the user now in plain \
                                    text from what you already have, or say what you could not \
                                    find. Do not write a tool call.";
/// Default heading above the message of yours a reply answers.
pub const REPLY_HEADER: &str = "The user replied to this earlier message of yours. It is what \
                                they are answering, so read it as the immediate context of what \
                                they say next.";
/// Default line closing a tool result cut to fit the prompt, with {chars}.
pub const RESULT_CUT_LINE: &str = "[{chars} more characters of this result were cut to fit. \
                                   Search again with a narrower query to read them.]";
/// Default heading above what the model may do.
pub const CAPABILITIES_HEADER: &str = "What you can do. Each line is a name, how it runs, and \
                                       what it does.";

impl Default for Templates<'_> {
    fn default() -> Self {
        Self {
            role_line: ROLE_LINE,
            role_line_no_roles: ROLE_LINE_NO_ROLES,
            memory_header: MEMORY_HEADER,
            capabilities_header: CAPABILITIES_HEADER,
            date_line: DATE_LINE,
            reply_header: REPLY_HEADER,
            result_cut_line: RESULT_CUT_LINE,
            memory_rules: MEMORY_RULES,
            discovery_rules: DISCOVERY_RULES,
        }
    }
}

/// Everything that can go into a prompt.
#[derive(Debug, Default)]
pub struct Sections<'a> {
    /// Versioned system instructions.
    pub system: &'a str,
    /// Recalled memories, best first.
    pub memory: &'a [Memory],
    /// One line per file the user attached. Never trimmed.
    pub uploads: &'a [String],
    /// Prior turns, oldest first.
    pub history: &'a [Message],
    /// Messages after the input: tool calls and results, oldest first, always kept.
    pub turn: &'a [Message],
    /// What the model may do, rendered. Empty writes no section.
    pub capabilities: &'a str,
    /// The current message from the user.
    pub input: &'a str,
    /// Today's date as the model should read it. Empty writes no date line.
    pub date: &'a str,
    /// Wording written around the sections.
    pub templates: Templates<'a>,
}

/// The assembled prompt plus what was left out.
#[derive(Debug)]
pub struct Assembled {
    /// Messages to send, in order.
    pub messages: Vec<Message>,
    /// Rough token estimate of the whole thing.
    pub estimated_tokens: usize,
    /// Memories that made it in, counted from the first.
    pub memory_used: usize,
}
