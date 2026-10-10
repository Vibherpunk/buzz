#!/bin/bash
sed -i.bak 's/const TIER1: &\[&str\] = &\["goose", "claude", "codex", "buzz-agent"\];/const TIER1: \&\[\&str\] = \&\["goose", "claude", "codex", "buzz-agent", "muse", "muse-code"\];/g' /Users/ai/workspace/buzz/desktop/src-tauri/src/managed_agents/custom_harnesses.rs
