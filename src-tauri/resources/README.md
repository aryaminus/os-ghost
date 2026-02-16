# OS Ghost Resources

## Gmail OAuth (Optional)

Place your Gmail OAuth client JSON here as "gmail_client_secret.json" for production builds.
Alternatively, provide OS_GHOST_GMAIL_CLIENT_ID, OS_GHOST_GMAIL_CLIENT_SECRET, OS_GHOST_GMAIL_PROJECT_ID, and OS_GHOST_GMAIL_REDIRECT_URI at runtime.

## Workspace Context Files (Moltis-Inspired)

Place these files in the data directory to customize agent behavior:

- **TOOLS.md** - Tool documentation and policies
- **AGENTS.md** - Agent-specific instructions
- **BOOT.md** - Startup context loaded on app launch

## AIEOS Identity (ZeroClaw-Inspired)

Place `identity.json` in the data directory to define the AI persona:

```json
{
  "identity": {
    "name": "Ghost",
    "bio": "A mysterious AI companion"
  },
  "psychology": {
    "traits": ["curious", "helpful"],
    "mbti": "INTJ"
  }
}
```

## TOML Configuration

For TOML-based configuration, create `config.toml` in the data directory.
