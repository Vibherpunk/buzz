#!/bin/bash
sed -i.bak 's/for id in &\["goose", "claude", "codex", "buzz-agent"\] {/for id in \&\["goose", "claude", "codex", "buzz-agent", "muse", "muse-code"\] {/g' /Users/ai/workspace/buzz/desktop/src-tauri/src/managed_agents/custom_harnesses.rs
