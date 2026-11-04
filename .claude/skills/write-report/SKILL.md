---
name: write-report
description: Report results, findings or research honestly and briefly, with every number and citation traced to its source. Use when you write up an experiment, benchmark, eval, investigation or literature lookup, or tell the person what a run found.
---

A report is read by someone deciding what to do next. Give them the answer, the evidence and its limits, and nothing else.

## Read before you report

- Read the actual outputs (logs, result files, per-item records), not only the summary numbers.
- Before any number, say what it rests on: the denominator, which items, which version or config.
- Read examples, not only rates. Name at least one item where the behavior happened and one where it did not, and check that the scoring read them as a person would.
- If the inputs or files were edited after the run, say so when you quote them.

## Every claim is checked

1. Every number comes from a file or tool output you read in this session. Put its source next to it (file path, run id, command). If you cannot point to a source, leave the number out.
2. Give a rate with its count and denominator: "12/40 (30%)", not "30%". Under ~30 items, call it a pilot.
3. Compare two conditions only on the same items, with the size of the difference and its uncertainty. Otherwise say the runs are not comparable.
4. Cite a paper or doc only if you read it in this session from its primary source (the paper itself, official docs, the publisher). Quote the exact words the claim rests on. Never cite a blog summary or your memory. If you could not get the source, write "not checked".
5. Quote examples exactly, with their id. Do not pick only the striking ones.

## Say only what the evidence carries

- Separate what was measured from what it suggests. Give the interpretation its own sentence, starting with "This suggests" or "One reading is".
- Claim exactly as much as was tested: this model, this version, these items, this setting.
- Do not infer intent or beliefs from outputs. Write "the model changed its answer", not "the model gave in".
- Put limitations in the report: sample size, scoring checks, confounds, and anything you could not run.
- A null or negative result is a result. Report it at the same length as a positive one.

## Structure

Lead with the answer, then the evidence for each claim, then the limits, then what to run next.

```
# <the question, as asked>
<one or two sentences: the answer, with its headline number and denominator>

## Evidence
<one short paragraph or table per claim, each number with its source>

## Limits
<bullets>

## Next
<the one run that would change the conclusion most>
```

At most three claims. Put the rest in an appendix, a table or the raw files.

## Plain, short sentences

- One idea per sentence, active voice, a specific verb.
- Define each term once and keep using it for the same thing.
- Begin each paragraph with its point.
- Use a table to compare numbers, a list for steps or options, and prose for the reasoning that connects them. Use a chart when a trend matters more than the exact values.
- Cut anything that does not change what the reader concludes or does: restatements of the question, summaries of the summary, praise of the method, and "future work" lists.
- Avoid filler and hype words: delve, crucial, pivotal, robust (unless measured), comprehensive, notably, interestingly, it is worth noting, plays a key role, landscape, showcase, underscore, seamless, novel (unless checked against prior work).
- Avoid filler patterns: groups of three for rhythm, "not just X but Y", rhetorical questions, bold scattered through prose, emoji, and closing paragraphs that restate the report.

## Before handing it over

Re-read it as a reviewer would:

- Check each number against its source.
- For each claim, ask what result would have contradicted it.
- Delete any sentence whose removal loses nothing.
- If you could not source a number, remove it or tell the person which numbers are unsourced. Do not reword a number to hide that.

Sources: Google's Technical Writing One (developers.google.com/tech-writing/one), and Neel Nanda's "Highly Opinionated Advice on How to Write ML Papers" (alignmentforum.org, 2025).
