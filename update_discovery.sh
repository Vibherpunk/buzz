#!/bin/bash
sed -i.bak '/KNOWN_ACP_RUNTIMES: &\[KnownAcpRuntime\] = &\[/a\
    KnownAcpRuntime {\
        id: "muse",\
        label: "Muse Code",\
        commands: &["muse-code-acp", "muse"],\
        aliases: &["muse-code"],\
        avatar_url: "",\
        mcp_command: None,\
        mcp_hooks: false,\
        underlying_cli: Some("muse"),\
        cli_install_commands: &[],\
        cli_install_commands_windows: &[],\
        adapter_install_commands: &[],\
        cli_install_instructions_url: "",\
        adapter_install_instructions_url: "",\
        cli_install_hint: "",\
        adapter_install_hint: "",\
        skill_dir: None,\
        supports_acp_model_switching: false,\
        model_env_var: Some("MUSE_MODEL"),\
        provider_env_var: Some("MUSE_PROVIDER"),\
        provider_locked: false,\
        default_env: &[],\
        config_file_path: None,\
        config_file_format: None,\
        supports_acp_native_config: false,\
        thinking_env_var: None,\
        max_tokens_env_var: None,\
        context_limit_env_var: None,\
        max_rounds_env_var: None,\
        required_normalized_fields: &["model"],\
        login_hint: None,\
        auth_probe_args: None,\
    },
' /Users/ai/workspace/buzz/desktop/src-tauri/src/managed_agents/discovery.rs
