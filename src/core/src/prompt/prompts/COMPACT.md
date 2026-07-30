Summarize the following conversation into a continuation-ready context for a coding agent.

Conversation:
{{CONVERSATION}}

Requirements:

- Produce a single concise but information-dense summary.
- Preserve all information needed to continue the conversation without rereading the original messages.
- Prioritize correctness over brevity. Omit only repetition, filler, and conversational niceties.

Include:

1. Overall objective
   - What the user is trying to accomplish.
   - Current status of the project.

2. Decisions and reasoning
   - Architectural decisions.
   - Tradeoffs discussed.
   - Constraints or assumptions.
   - Any plans that were agreed upon.

3. Code and implementation details
   - File paths.
   - Functions, classes, APIs, schemas, interfaces, and important code snippets.
   - Configuration changes.
   - Commands that matter.
   - Dependencies and versions when relevant.

4. Work completed
   - Brief chronological summary of meaningful work.
   - What changed and why.

5. Bugs and debugging
   - Errors encountered.
   - Root causes.
   - Fixes attempted.
   - Remaining issues.

6. Important references
   - Relevant URLs.
   - Documentation referenced.
   - External resources.
   - File locations.

7. Current state
   - What currently works.
   - What is unfinished.
   - Outstanding TODOs.
   - Suggested next step.

Guidelines:

- Preserve exact names of files, functions, variables, commands, environment variables, APIs, and identifiers.
- Include short code snippets only when the exact syntax matters.
- Keep implementation details that would be expensive to rediscover.
- Do not summarize away technical decisions.
- Remove repetition, explanations, and conversational text.
- Write as if handing the project to another engineer who must continue immediately.
- Do not mention the summarization process.
- Output only the summary.
