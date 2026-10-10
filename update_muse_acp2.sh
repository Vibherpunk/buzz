#!/bin/bash
sed -i.bak 's/if (process.env.MUSE_MODEL) { args.unshift('"'"'--model'"'"', process.env.MUSE_MODEL); }/if (process.env.MUSE_MODEL) { args.unshift('"'"'--model'"'"', process.env.MUSE_MODEL); }\n    if (process.env.MUSE_PROVIDER) { args.unshift('"'"'--provider'"'"', process.env.MUSE_PROVIDER); }/g' /Users/ai/.local/bin/muse-code-acp
