---
name: technical-writing
description: Write docs, READMEs, comments, docstrings, error messages and PR text in plain technical English, based on ASD-STE100 Simplified Technical English. Use whenever you write or edit prose in a repository.
---

Write prose by the rules of ASD-STE100 (Simplified Technical English), adapted for software. The goal is text that a reader with basic English can follow without doubt.

## Words

- Use one word for one meaning, and use it every time. If the project has a glossary (GLOSSARY.md, CONTEXT.md, or a glossary section in the docs), use its terms. Do not change words for variety.
- Use simple, common verbs: use, get, set, make, run, start, stop, show, find, add, remove, keep, send, read, write.
- Do not use a verb as a noun or a noun as a verb ("do the install" is wrong; "install it" is correct).
- Do not use words with no exact meaning: simply, just, easily, basically, robust, seamless, powerful, leverage, utilize, various, etc.
- Code names (files, functions, env vars, routes) go in backticks and are not changed.

## Sentences

- Instructions: 20 words or fewer. Descriptions: 25 words or fewer.
- Write one instruction in each sentence. When two actions occur at the same time, you can write them in one sentence.
- Use the imperative for instructions: "Run `make test`." Not "You should run" or "Running".
- Use the active voice. Write who or what does the action.
- Use the present tense for what the code does: "The job posts new items."
- Put a condition first: "If the token is expired, the route returns 401."
- Write warnings and cautions before the step they apply to, and say the result if the reader does not obey.
- Do not use -ing words as a verb or a noun ("Restarting the pod fixes it" is wrong; "Restart the pod" is correct).

## Paragraphs and pages

- One topic in each paragraph. Six sentences or fewer.
- Put procedures in numbered steps. Put items with no order in a bullet list.
- Use a table for reference data (env vars, routes, files).
- When a flow, structure or relationship is easier to see than to read, draw it (Mermaid or a small ASCII tree) and keep the prose around it short.
- Start a page with what the thing does, in one or two sentences. No history, no marketing.
- A page has only what a reader needs to use or change the thing. Link to code for the rest.
- Do not over-format. Use headings only when the page has more than one topic. Do not bold for emphasis.

## Code comments

- Plain ASCII. Write what the code does or the fact it depends on. The why goes in the commit message.
- One line when one line is sufficient. No comment when the name says it.
- A docstring starts with a short description of the result: "The members of an org, newest first."
- No commented-out code. No TODO without an issue link.

## Check before you finish

1. Read each sentence. Remove each word that does not change the meaning.
2. Count the words in long sentences. Split a sentence that is too long.
3. Make sure each term matches the project's glossary, if it has one.
