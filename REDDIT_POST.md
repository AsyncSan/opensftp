# Reddit Post — r/programming

**Subreddit:** r/programming  
**Status:** Ready to post

---

## Title

What I learned building a Rust desktop app with agentic AI as the primary implementer

---

## Body

I just shipped **Open SFTP** - a native SFTP client in Rust. The technical story is interesting but the development workflow is what I want to share.

I used agentic AI (Claude Code) for most of the implementation. Not copilot-style autocomplete - the agent was given tasks and executed them: write this feature, fix this bug, add these tests.

**What actually worked:**

1. **Tight task scoping.** The agent is great when the task is a single, well-defined unit. "Add a progress bar to the transfer dialog" = solid output. "Implement file transfer" = unusable soup.

2. **Rust-specific reliability.** I expected borrow checker errors to be a constant fight. Surprisingly, the agent handled lifetime and ownership issues well when tasks were small. It rewrites the function, not the architecture.

3. **Test generation.** 263 tests, mostly agent-written. They caught 4-5 real bugs I would have missed.

**What broke down:**

- Any task requiring broad architectural judgment
- Anything involving cross-module state changes without a clear spec
- Performance tuning (the agent tends to "fix" things by adding abstraction layers)

**The mental model that works:**
Treat the agent like a very fast junior engineer. They need clear tickets. They do not replace the architect. You still need to understand the codebase well enough to review the output.

App: https://renemurrell.de/software/opensftp  
GitHub: https://github.com/mylilcrowdi/Open-SFTP-Rust
