# region: install
import agent_harness_adapter as aha

mytool = {
    "version": 1,
    "name": "mytool",
    "instructions": "Run `mytool check` before you finish.\n",
    "skills": [{"name": "mytool", "description": "Check the project with mytool.",
                "body": "Run `mytool check`; fix what it reports.\n"}],
    "hooks": [{"event": "pre-tool", "tools": "shell", "command": "mytool hook {harness} {event}"}],
    "allow_commands": ["mytool"],
}

result = aha.install(mytool, ["claude", "codex", "gemini"])
for h in result["harnesses"]:
    print(h["harness"], [f'{p["state"]} {p["path"]}' for p in h["parts"]])
# endregion: install
