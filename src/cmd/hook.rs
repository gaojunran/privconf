use anyhow::anyhow;

pub fn run(shell: &str) -> anyhow::Result<()> {
    let script = match shell {
        "bash" => BASH_HOOK,
        "zsh" => ZSH_HOOK,
        "fish" => FISH_HOOK,
        _ => return Err(anyhow!("unsupported shell: {shell}. Supported: bash, zsh, fish")),
    };
    println!("{script}");
    Ok(())
}

const BASH_HOOK: &str = r#"
__privconf_chpwd_hook() {
    if [ "$__PRIVCONF_LAST_DIR" = "$PWD" ]; then
        return
    fi
    export __PRIVCONF_LAST_DIR="$PWD"
    privconf link --quiet 2>/dev/null || true
}
if ! [[ "${PROMPT_COMMAND:-}" == *__privconf_chpwd_hook* ]]; then
    PROMPT_COMMAND="__privconf_chpwd_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
"#;

const ZSH_HOOK: &str = r#"
__privconf_chpwd_hook() {
    if [ "$__PRIVCONF_LAST_DIR" = "$PWD" ]; then
        return
    fi
    export __PRIVCONF_LAST_DIR="$PWD"
    privconf link --quiet 2>/dev/null || true
}
chpwd_functions=(__privconf_chpwd_hook "${chpwd_functions[@]}")
"#;

const FISH_HOOK: &str = r#"
function __privconf_chpwd_hook --on-variable PWD
    privconf link --quiet 2>/dev/null; or true
end
"#;
