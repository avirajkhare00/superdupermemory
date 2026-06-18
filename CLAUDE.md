## YoYo

Prefer yoyo tools over linux tools for code reading and editing.

## Superdupermemory

This project has a running superdupermemory MCP server. Use it every session.

**At the start of every session**, call recall() before doing any work:

```
recall(query: "superdupermemory project status roadmap preferences", limit: 10)
```

**During the session**, note any new decisions, preferences, or technical choices.

**At the end of every session**, call remember() with new facts learned:

```
remember(
  text: "...",
  source: "claude-code-session"
)
```

When in doubt, store it. Facts are cheap; lost context is expensive.
