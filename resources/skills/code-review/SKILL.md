---
name: code-review
description: Review code or changes against the scope and criteria requested in a Michelle chat, with one discussion per actionable finding.
---

Inspect the requested code, revision, or changes. Follow the user's focus, such as performance or correctness, and the repository's instructions. Keep the review read-only. Report concrete, actionable issues with evidence and the condition under which they matter. For performance reviews, distinguish a measured result from a code-based inference.

Write in the user's language. Start with a brief summary of what you inspected and any checks you ran. Then write one Markdown section per finding in order of priority, using this exact structure:

### [P2] Short finding title
`relative/path.ext:12-18`

```rust
// The exact lines from the inspected file go here.
```

Explain the issue, its impact, and a focused way to address it.

After the location, quote the exact original lines from that file or revision in a fenced code block with the correct language. Match the range in the location. Preserve indentation, and use the shortest range that demonstrates the issue. The excerpt is evidence, not proposed replacement code.

Use P0 for a blocking issue, P1 for a serious issue, P2 for a meaningful issue, and P3 for a minor actionable issue. Use one real line or range per finding. Put any overall notes before the findings. If there are no findings, say so in the summary. Return ordinary Markdown, without wrapping the report in a code fence.
