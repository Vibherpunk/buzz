#!/bin/bash
sed -i.bak 's/const child = spawn('"'"'\/Users\/ai\/.local\/bin\/muse'"'"', \['"'"'exec'"'"', promptText\], {/const args = \['"'"'exec'"'"', promptText\];\n    if (process.env.MUSE_MODEL) { args.unshift('"'"'--model'"'"', process.env.MUSE_MODEL); }\n    const child = spawn('"'"'\/Users\/ai\/.local\/bin\/muse'"'"', args, {/g' /Users/ai/.local/bin/muse-code-acp
