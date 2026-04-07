# PITCH.md — CLI-First Demo Script & Judge Q&A

## The 2.5-Minute Pitch

### [0:00-0:15] The Hook
"Every 14 minutes, a developer leaks AI intellectual property to public GitHub. Most tools scan after push. Provn stops that leak before the commit lands, fully locally."

### [0:15-0:50] The Demo — The Block
*Open terminal*

"Here’s a staged change with a hidden system prompt and a credential."

*Run: `git commit -m "add feature"`*

*Terminal shows a block with file, line, tier, and redacted preview*

"Provn caught it before the code left the laptop."

### [0:50-1:15] The Demo — The Fix
*Show auto-redact preview*

"Provn doesn’t just say no. It shows exactly what is dangerous and offers a safe replacement."

*Accept the redaction or fix the file and re-run the commit*

"Now the commit passes."

### [1:15-1:45] The Proof
*Show benchmark output or report*

"We measure this against curated leak cases, not hand-wavy examples."

- strong recall on seeded leaks
- low false positive rate
- fast clean-commit latency

"And the semantic layer is optional, local, and fail-safe."

### [1:45-2:10] The Trust Story
"Every decision is written to a tamper-evident audit chain. If someone bypasses the hook, we can still verify what happened later."

*Run: `provn verify-audit`*

"That gives teams both prevention and accountability."

### [2:10-2:30] The Close
"Provn is a local-first AI security guard for your repo: fast, auditable, and built for the era of prompt leaks and secret spills."

---

## Judge Q&A Cheat Sheet

### "What if I disable the hook?"
"Provn is designed to make bypasses visible through audit logging and CI re-checks. The local hook is the first line of defense, not the only one."

### "How do you handle false positives?"
"We use layered detection, configurable policy tiers, and LeakBench-style evaluation so we can tune against real examples instead of guessing."

### "Why local-first vs cloud?"
"If you are protecting sensitive code, sending it to a cloud scanner is a contradiction. Provn keeps scanning on the machine by default."

### "How is this different from GitHub Secret Protection?"
"GitHub scans after push. Provn scans before commit. That changes the blast radius completely."

### "Why Gemma 4?"
"It gives us a local semantic layer that can run on developer hardware without requiring a hosted inference API."

### "Why CLI-first?"
"Because the CLI is the real product wedge. It is the fastest path to protection, lowest maintenance surface, and easiest thing for a team to adopt."
